"""
File-level backup encryption for ServerKit.

Client-side encryption of backup artifacts (DB dumps, config archives, file
bundles) using Fernet with the same ``SERVERKIT_ENCRYPTION_KEY`` that protects
secrets at rest. Encrypting here means a backup is already ciphertext before it
is auto-uploaded to remote storage, so the panel never trusts the remote with
plaintext data.

Reuses :mod:`app.utils.crypto` — no new crypto dependencies. Key loss means the
encrypted backups are unrecoverable, so the key must be backed up out of band.
"""

import os

from cryptography.fernet import Fernet

from app.utils.crypto import get_encryption_key, is_encryption_configured

# Suffix appended to an encrypted backup artifact.
ENC_SUFFIX = '.enc'


def encryption_available() -> bool:
    """Return True when backup encryption can be performed.

    Delegates to :func:`app.utils.crypto.is_encryption_configured`, i.e. true
    only when ``SERVERKIT_ENCRYPTION_KEY`` is set. The derived development key
    is intentionally not treated as "available" so dev backups are not silently
    encrypted with a non-secret key.
    """
    return is_encryption_configured()


def encrypt_file(path: str) -> str:
    """Encrypt a file in place, returning the path of the encrypted artifact.

    Reads the file bytes, Fernet-encrypts them, writes the ciphertext to
    ``path + '.enc'``, removes the original plaintext file, and returns the new
    path.

    Note: this loads the entire file into memory. That is fine for typical
    database dumps and config archives; streaming encryption for very large
    archives is a follow-up.

    Args:
        path: Path to the plaintext file to encrypt.

    Returns:
        str: Path to the encrypted file (``path + '.enc'``).
    """
    f = Fernet(get_encryption_key())

    with open(path, 'rb') as src:
        data = src.read()

    token = f.encrypt(data)

    enc_path = path + ENC_SUFFIX
    with open(enc_path, 'wb') as dst:
        dst.write(token)

    os.remove(path)
    return enc_path


def decrypt_file(path: str, dest: str = None) -> str:
    """Decrypt a ``.enc`` backup file, returning the path of the plaintext.

    Args:
        path: Path to the encrypted ``.enc`` file.
        dest: Optional destination path. If None, the trailing ``.enc`` is
            stripped from ``path`` to form the destination.

    Returns:
        str: Path to the decrypted file.

    Raises:
        cryptography.fernet.InvalidToken: If the key is wrong or data corrupted.
    """
    if dest is None:
        if path.endswith(ENC_SUFFIX):
            dest = path[:-len(ENC_SUFFIX)]
        else:
            dest = path + '.dec'

    f = Fernet(get_encryption_key())

    with open(path, 'rb') as src:
        token = src.read()

    data = f.decrypt(token)

    with open(dest, 'wb') as dst:
        dst.write(data)

    return dest


def is_encrypted_backup(path: str) -> bool:
    """Return True when ``path`` looks like an encrypted backup artifact."""
    return bool(path) and path.endswith(ENC_SUFFIX)
