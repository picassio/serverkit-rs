"""Encrypted secrets manager (vault + secret) service."""
import json
import logging
import re
import secrets
from datetime import datetime
from typing import Dict, List, Optional

from app import db
from app.models import Secret, SecretVault
from app.utils.crypto import encrypt_secret, decrypt_secret_safe
from app.utils.slug import unique_slug

logger = logging.getLogger(__name__)


def _unique_slug(name: str) -> str:
    return unique_slug(
        name,
        lambda s: SecretVault.query.filter_by(slug=s).first() is not None,
        default='vault',
    )


class SecretVaultService:
    """Manage encrypted secret vaults."""

    @classmethod
    def list_vaults(cls, workspace_id: int = None) -> List[Dict]:
        query = SecretVault.query
        if workspace_id is not None:
            query = query.filter(SecretVault.workspace_id == workspace_id)
        return [v.to_dict() for v in query.order_by(SecretVault.name).all()]

    @classmethod
    def get_vault(cls, vault_id: int) -> Optional[SecretVault]:
        return SecretVault.query.get(vault_id)

    @classmethod
    def create_vault(cls, name: str, description: str = None, user_id: int = None,
                     workspace_id: int = None) -> Dict:
        if SecretVault.query.filter_by(name=name).first():
            return {'success': False, 'error': 'Vault name already exists'}
        vault = SecretVault(
            name=name,
            slug=_unique_slug(name),
            description=description,
            created_by=user_id,
            workspace_id=workspace_id,
        )
        db.session.add(vault)
        db.session.commit()
        return {'success': True, 'vault': vault.to_dict()}

    @classmethod
    def update_vault(cls, vault_id: int, name: str = None, description: str = None) -> Dict:
        vault = cls.get_vault(vault_id)
        if not vault:
            return {'success': False, 'error': 'Vault not found'}
        if name is not None:
            existing = SecretVault.query.filter(SecretVault.name == name, SecretVault.id != vault_id).first()
            if existing:
                return {'success': False, 'error': 'Vault name already exists'}
            vault.name = name
        if description is not None:
            vault.description = description
        db.session.commit()
        return {'success': True, 'vault': vault.to_dict()}

    @classmethod
    def delete_vault(cls, vault_id: int) -> Dict:
        vault = cls.get_vault(vault_id)
        if not vault:
            return {'success': False, 'error': 'Vault not found'}
        db.session.delete(vault)
        db.session.commit()
        return {'success': True, 'message': 'Vault deleted'}


class SecretService:
    """Manage encrypted secrets inside vaults."""

    _NAME_RE = re.compile(r'^[A-Z_][A-Z0-9_]*$', re.IGNORECASE)

    @classmethod
    def list_secrets(cls, vault_id: int) -> List[Dict]:
        return [s.to_dict(include_value=True, mask=True) for s in Secret.query.filter_by(vault_id=vault_id).order_by(Secret.name).all()]

    @classmethod
    def get_secret(cls, secret_id: int) -> Optional[Secret]:
        return Secret.query.get(secret_id)

    @classmethod
    def get_secret_by_name(cls, vault_id: int, name: str) -> Optional[Secret]:
        return Secret.query.filter_by(vault_id=vault_id, name=name).first()

    @classmethod
    def _validate_name(cls, name: str) -> Optional[str]:
        if not name:
            return 'Name is required'
        if not cls._NAME_RE.match(name):
            return 'Name must start with a letter or underscore and contain only letters, digits, and underscores'
        return None

    @classmethod
    def create_secret(cls, vault_id: int, name: str, value: str,
                      description: str = None, expires_at=None) -> Dict:
        if not SecretVault.query.get(vault_id):
            return {'success': False, 'error': 'Vault not found'}
        err = cls._validate_name(name)
        if err:
            return {'success': False, 'error': err}
        if Secret.query.filter_by(vault_id=vault_id, name=name).first():
            return {'success': False, 'error': 'Secret name already exists in this vault'}
        try:
            encrypted = encrypt_secret(value)
        except Exception as e:
            return {'success': False, 'error': str(e)}
        secret = Secret(
            vault_id=vault_id,
            name=name,
            encrypted_value=encrypted,
            description=description,
            expires_at=expires_at,
        )
        db.session.add(secret)
        db.session.commit()
        return {'success': True, 'secret': secret.to_dict(include_value=True, mask=True)}

    @classmethod
    def update_secret(cls, secret_id: int, value: str = None, description: str = None,
                      expires_at=None, rotate: bool = False) -> Dict:
        secret = cls.get_secret(secret_id)
        if not secret:
            return {'success': False, 'error': 'Secret not found'}
        if value is not None:
            try:
                secret.encrypted_value = encrypt_secret(value)
            except Exception as e:
                return {'success': False, 'error': str(e)}
            secret.updated_at = datetime.utcnow()
        if description is not None:
            secret.description = description
        if expires_at is not None:
            secret.expires_at = expires_at
        if rotate:
            decrypted = secret.value
            if decrypted:
                try:
                    secret.encrypted_value = encrypt_secret(decrypted)
                except Exception as e:
                    return {'success': False, 'error': str(e)}
                secret.updated_at = datetime.utcnow()
        db.session.commit()
        return {'success': True, 'secret': secret.to_dict(include_value=True, mask=True)}

    @classmethod
    def delete_secret(cls, secret_id: int) -> Dict:
        secret = cls.get_secret(secret_id)
        if not secret:
            return {'success': False, 'error': 'Secret not found'}
        db.session.delete(secret)
        db.session.commit()
        return {'success': True, 'message': 'Secret deleted'}

    @classmethod
    def reveal_secret(cls, secret_id: int) -> Dict:
        secret = cls.get_secret(secret_id)
        if not secret:
            return {'success': False, 'error': 'Secret not found'}
        value = secret.value
        if value is None:
            return {'success': False, 'error': 'Unable to decrypt secret'}
        return {'success': True, 'secret': secret.to_dict(include_value=True, mask=False)}

    @classmethod
    def bulk_create_or_update(cls, vault_id: int, secrets_list: List[Dict]) -> Dict:
        """Create or update many secrets. Each item needs name and value."""
        if not SecretVault.query.get(vault_id):
            return {'success': False, 'error': 'Vault not found'}
        results = []
        errors = []
        for item in secrets_list:
            name = item.get('name')
            value = item.get('value')
            if not name or value is None:
                errors.append({'name': name, 'error': 'name and value required'})
                continue
            existing = cls.get_secret_by_name(vault_id, name)
            if existing:
                result = cls.update_secret(existing.id, value=value)
            else:
                result = cls.create_secret(vault_id, name, value, description=item.get('description'))
            if result.get('success'):
                results.append(result['secret'])
            else:
                errors.append({'name': name, 'error': result.get('error')})
        return {'success': True, 'secrets': results, 'errors': errors}

    @classmethod
    def resolve_env_dict(cls, vault_id: int, prefix: str = '') -> Dict[str, str]:
        """Resolve vault secrets to an env-style dict (server-side use only)."""
        secrets = Secret.query.filter_by(vault_id=vault_id).all()
        env = {}
        for s in secrets:
            if s.is_expired:
                continue
            value = s.value
            if value is None:
                continue
            key = f'{prefix}{s.name}'
            env[key] = value
        return env
