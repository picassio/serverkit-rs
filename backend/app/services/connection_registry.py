"""Unified read facade over every external-account store ServerKit holds.

ServerKit's connections live in five places, each with its own model/service and
write path (kept that way deliberately — they have genuinely different auth and
cardinality):

  - source connections   (SourceConnection)      — per-user OAuth (GitHub, GitLab)
  - DNS providers         (DNSProviderConfig)     — Cloudflare / Route 53 / DO / GoDaddy
  - cloud providers       (CloudProvider)         — DigitalOcean / Hetzner / Vultr / Linode
  - registrars            (RegistrarConnection)   — GoDaddy / Namecheap
  - object storage        (storage.json)          — S3-compatible / Backblaze B2

This registry doesn't replace any of them; it presents them as ONE normalized,
secret-free list (`GET /api/v1/connections`) so any surface can answer "what
external accounts are connected, and are their secrets encrypted?" in one call.
All five now encrypt secrets identically via app.utils.crypto, so `encrypted`
should be True for every real connection.
"""

import logging

logger = logging.getLogger(__name__)


def _entry(kind, provider, label, *, id=None, scope=None, encrypted=True, created_at=None):
    return {
        'kind': kind,
        'provider': provider,
        'id': id,
        'label': label,
        'scope': scope,
        'encrypted': bool(encrypted),
        'created_at': created_at.isoformat() if hasattr(created_at, 'isoformat') else created_at,
    }


class ConnectionRegistry:
    """Read-only aggregator. Returns plain dicts with no secret material."""

    @classmethod
    def list_all(cls, user_id=None):
        out = []
        for fn in (cls._source, cls._dns, cls._infra, cls._registrar, cls._storage, cls._email, cls._registries):
            try:
                out += fn(user_id) if fn is cls._source else fn()
            except Exception as e:  # one failing store never breaks the whole list
                logger.warning(f'ConnectionRegistry: {fn.__name__} failed: {e}')
        return out

    @staticmethod
    def _source(user_id=None):
        from app.models.source_connection import SourceConnection
        q = SourceConnection.query
        if user_id is not None:
            q = q.filter_by(user_id=user_id)
        return [
            _entry('source', c.provider,
                   c.display_name or c.provider_username or c.provider,
                   id=c.id, scope='OAuth',
                   encrypted=bool(c.access_token_encrypted), created_at=c.created_at)
            for c in q.all()
        ]

    @staticmethod
    def _dns():
        from app.models.email import DNSProviderConfig
        from app.utils.crypto import is_encrypted
        out = []
        for c in DNSProviderConfig.query.all():
            scope = 'Global key' if (c.provider == 'cloudflare' and c.api_email) else 'API key'
            out.append(_entry('dns', c.provider, c.name, id=c.id, scope=scope,
                              encrypted=is_encrypted(c.api_key or ''), created_at=c.created_at))
        return out

    @staticmethod
    def _infra():
        from app.models.cloud_server import CloudProvider
        from app.utils.crypto import is_encrypted
        return [
            _entry('infra', c.provider_type, c.name, id=c.id, scope='Account token',
                   encrypted=is_encrypted(c.api_key_encrypted or ''), created_at=c.created_at)
            for c in CloudProvider.query.filter_by(is_active=True).all()
        ]

    @staticmethod
    def _registrar():
        from app.models.registrar_connection import RegistrarConnection
        return [
            _entry('registrar', c.provider, c.name or c.provider, id=c.id, scope='API key',
                   encrypted=bool(c.api_key_encrypted), created_at=c.created_at)
            for c in RegistrarConnection.query.all()
        ]

    @staticmethod
    def _email():
        from app.models.email_provider import EmailProviderConnection
        out = []
        for c in EmailProviderConnection.query.all():
            scope = 'SMTP' if c.provider == 'smtp' else 'API key'
            if c.is_default:
                scope += ' · default'
            out.append(_entry('email', c.provider, c.name, id=c.id, scope=scope,
                              encrypted=True, created_at=c.created_at))
        return out

    @staticmethod
    def _registries():
        from app.models.container_registry import ContainerRegistry
        return [
            _entry('registry', c.provider, c.name, id=c.id, scope=c.login_host(),
                   encrypted=bool(c.secret_encrypted), created_at=c.created_at)
            for c in ContainerRegistry.query.all()
        ]

    @staticmethod
    def _storage():
        from app.services.storage_provider_service import StorageProviderService
        cfg = StorageProviderService.get_config()
        provider = cfg.get('provider')
        if provider in ('s3', 'b2'):
            bucket = (cfg.get(provider) or {}).get('bucket')
            if bucket:
                return [_entry('storage', provider, f'{provider.upper()} · {bucket}',
                               scope='Access key', encrypted=True)]
        return []
