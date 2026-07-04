"""
Cryptographic utilities for ServerKit.

Provides encryption/decryption for sensitive data like API secrets.
"""

import os
import base64
import re
import warnings
import logging
from pathlib import Path
from cryptography.fernet import Fernet, InvalidToken
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.kdf.pbkdf2 import PBKDF2HMAC

logger = logging.getLogger(__name__)


def _find_env_file() -> Path:
    """Locate the ServerKit .env file on the host or in containers."""
    # Explicit override takes precedence even if the file does not yet exist,
    # so installers/tests can target a specific path.
    env_path = os.environ.get('SERVERKIT_ENV_FILE')
    if env_path:
        return Path(env_path)

    candidates = []

    # SERVERKIT_INSTALL_DIR is rendered into the systemd unit from the
    # installer's SERVERKIT_DIR — the unambiguous "where is the install" var
    # (the bare SERVERKIT_DIR name doubles as the /var/serverkit data root in
    # app/paths.py, so it stays a lower-priority hint here).
    install_dir = os.environ.get('SERVERKIT_INSTALL_DIR')
    if install_dir:
        candidates.append(Path(install_dir) / '.env')

    # SERVERKIT_DIR is used by the CLI and install script
    serverkit_dir = os.environ.get('SERVERKIT_DIR')
    if serverkit_dir:
        candidates.append(Path(serverkit_dir) / '.env')

    # Project-root .env (works in dev and when installed to /opt/serverkit)
    # __file__ is backend/app/utils/crypto.py -> project root is 3 levels up
    candidates.append(Path(__file__).resolve().parents[2] / '.env')

    # Container layout fallback
    candidates.append(Path('/opt/serverkit/.env'))

    for candidate in candidates:
        if candidate.exists():
            return candidate

    # Default to project-root .env even if it doesn't exist, so callers can create it
    return candidates[0] if candidates else Path('/opt/serverkit/.env')


def generate_encryption_key() -> str:
    """Generate a new Fernet encryption key."""
    return Fernet.generate_key().decode()


def write_encryption_key_to_env(key: str, env_file: Path = None) -> Path:
    """Persist SERVERKIT_ENCRYPTION_KEY to the .env file, creating it if needed."""
    if env_file is None:
        env_file = _find_env_file()

    env_file = Path(env_file)
    env_file.parent.mkdir(parents=True, exist_ok=True)

    content = env_file.read_text() if env_file.exists() else ''

    # Replace existing key or append
    pattern = re.compile(r'^[ \t]*SERVERKIT_ENCRYPTION_KEY[ \t]*=.*$', re.MULTILINE)
    new_line = f'SERVERKIT_ENCRYPTION_KEY={key}'

    if pattern.search(content):
        content = pattern.sub(new_line, content)
    else:
        if content and not content.endswith('\n'):
            content += '\n'
        content += f'\n# Encryption key for agent pairing payloads and secrets at rest\n{new_line}\n'

    env_file.write_text(content)
    return env_file


def ensure_encryption_key() -> bytes:
    """Ensure an encryption key exists in production.

    If SERVERKIT_ENCRYPTION_KEY is missing in production, auto-generate one and
    write it to the .env file. This prevents first-time pairing crashes while
    still keeping secrets encrypted at rest.

    Returns:
        bytes: The encryption key
    """
    key = os.environ.get('SERVERKIT_ENCRYPTION_KEY')
    if key:
        return key.encode()

    if os.environ.get('FLASK_ENV') != 'production':
        # Dev/test: fall through to the derived key below
        return None

    logger.warning(
        'SERVERKIT_ENCRYPTION_KEY is not set in production. '
        'Auto-generating one and writing it to .env. '
        'This is normal for new installs.'
    )
    new_key = generate_encryption_key()
    try:
        env_file = write_encryption_key_to_env(new_key)
        os.environ['SERVERKIT_ENCRYPTION_KEY'] = new_key
        logger.info('SERVERKIT_ENCRYPTION_KEY written to %s', env_file)
    except Exception as exc:
        logger.error('Failed to persist SERVERKIT_ENCRYPTION_KEY: %s', exc)
        raise ValueError(
            'SERVERKIT_ENCRYPTION_KEY must be set in production and could not be auto-generated. '
            'Generate one with: python -c "from cryptography.fernet import Fernet; print(Fernet.generate_key().decode())"'
        ) from exc

    return new_key.encode()


def get_encryption_key() -> bytes:
    """
    Get the encryption key from environment variable.

    The key should be a valid Fernet key (32 url-safe base64-encoded bytes).
    Generate one with: python -c "from cryptography.fernet import Fernet; print(Fernet.generate_key().decode())"

    Returns:
        bytes: The encryption key

    Raises:
        ValueError: If SERVERKIT_ENCRYPTION_KEY is not set and cannot be auto-generated
    """
    key = os.environ.get('SERVERKIT_ENCRYPTION_KEY')
    if not key:
        # In production, try to auto-generate and persist the key
        auto_key = ensure_encryption_key()
        if auto_key is not None:
            return auto_key

        # Development / testing fallback (NOT for production!)
        logger.warning('SECURITY WARNING: Using derived development encryption key. Set SERVERKIT_ENCRYPTION_KEY for production.')
        warnings.warn('Using derived development encryption key - not suitable for production')
        default_key = "DEV_ONLY_NOT_SECURE_CHANGE_IN_PRODUCTION_KEY"
        kdf = PBKDF2HMAC(
            algorithm=hashes.SHA256(),
            length=32,
            salt=b'serverkit_dev_salt',
            iterations=100000,
        )
        key_bytes = base64.urlsafe_b64encode(kdf.derive(default_key.encode()))
        return key_bytes
    return key.encode()


def encrypt_secret(plaintext: str) -> str:
    """
    Encrypt a secret using Fernet symmetric encryption.

    Args:
        plaintext: The secret to encrypt

    Returns:
        str: Base64-encoded encrypted data
    """
    key = get_encryption_key()
    f = Fernet(key)
    encrypted = f.encrypt(plaintext.encode())
    return encrypted.decode()


def decrypt_secret(encrypted: str) -> str:
    """
    Decrypt a secret encrypted with encrypt_secret.

    Args:
        encrypted: Base64-encoded encrypted data

    Returns:
        str: The decrypted plaintext

    Raises:
        InvalidToken: If decryption fails (wrong key or corrupted data)
    """
    key = get_encryption_key()
    f = Fernet(key)
    decrypted = f.decrypt(encrypted.encode())
    return decrypted.decode()


def is_encryption_configured() -> bool:
    """
    Check if encryption is properly configured.

    Returns:
        bool: True if SERVERKIT_ENCRYPTION_KEY is set
    """
    return os.environ.get('SERVERKIT_ENCRYPTION_KEY') is not None


def decrypt_secret_safe(value: str) -> str:
    """Decrypt a value encrypted with ``encrypt_secret``; if it isn't valid
    ciphertext (e.g. a legacy plaintext secret not yet migrated), return it
    unchanged. Lets encrypted and not-yet-migrated values coexist during the
    transition to encryption-at-rest.
    """
    if not value:
        return value
    try:
        return decrypt_secret(value)
    except Exception:
        return value


def is_encrypted(value: str) -> bool:
    """Best-effort check that ``value`` is our Fernet ciphertext (so migrations
    can skip already-encrypted values). A real plaintext secret will not decrypt,
    so this is safe in practice."""
    if not value:
        return False
    try:
        decrypt_secret(value)
        return True
    except Exception:
        return False
