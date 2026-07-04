"""Outbound SMTP relay (smarthost) for the mail server.

The relay is now one row of the unified ``EmailProviderConnection`` table — an
``smtp`` connection flagged ``uses_relay`` (§6 unification) — so a single store
drives both the Notification Bus and the Postfix relay. The legacy single-row
``email_relay_config`` is migrated in on first boot and kept only as a read
fallback during the deprecation window.

Config is held platform-agnostically so the dev panel can hold it; the Postfix
apply is a no-op (with a note) where Postfix isn't installed. ``test()`` opens a
real SMTP connection so credentials validate on any OS.
"""

import ssl
import json
import smtplib
import logging

from app import db
from app.models.email import EmailRelayConfig
from app.models.email_provider import EmailProviderConnection
from app.notifications.providers import EmailProviderService
from app.services.postfix_service import PostfixService
from app.utils.crypto import encrypt_secret

logger = logging.getLogger(__name__)

# The relay connection is created/owned by this service.
RELAY_PROVIDER_NAME = 'Postfix Relay'


def _is_masked(value):
    """True for our masking sentinels, so a round-tripped masked value never
    overwrites the stored secret."""
    if not value:
        return False
    return value.startswith('****') or set(value) == {'*'}


def _coerce_port(value, default=587):
    try:
        return int(value or default)
    except (TypeError, ValueError):
        return default


def _coerce_tls(value):
    if isinstance(value, str):
        return value.lower() not in ('0', 'false', 'no')
    return bool(value) if value is not None else True


class EmailRelayService:
    DEFAULT = {
        'enabled': False, 'host': '', 'port': 587, 'username': '',
        'use_tls': True, 'provider_hint': None, 'password_set': False,
    }

    # ------------------------------------------------------------------ #
    # The relay connection (EmailProviderConnection, uses_relay=True)
    # ------------------------------------------------------------------ #
    @staticmethod
    def _relay_row():
        """The canonical relay connection regardless of active state (for
        display/edit). Active-only selection for *applying* lives in
        EmailProviderService.relay_provider()."""
        return (EmailProviderConnection.query
                .filter_by(uses_relay=True)
                .order_by(EmailProviderConnection.relay_priority.desc(),
                          EmailProviderConnection.created_at.asc())
                .first())

    @staticmethod
    def _view_from_provider(row):
        """Public relay view from a connection — never exposes the password."""
        creds = row.credentials()  # decrypted; non-secret fields pass through
        return {
            'enabled': bool(row.is_active),
            'host': creds.get('host') or '',
            'port': _coerce_port(creds.get('port')),
            'username': creds.get('username') or '',
            'use_tls': _coerce_tls(creds.get('use_tls')),
            'provider_hint': creds.get('provider_hint'),
            'password_set': bool(creds.get('password')),
            'updated_at': row.updated_at.isoformat() if row.updated_at else None,
            'provider_id': row.id,
        }

    @classmethod
    def get_config(cls):
        row = cls._relay_row()
        if row:
            return cls._view_from_provider(row)
        # Back-compat: a legacy single-row config not yet migrated.
        legacy = EmailRelayConfig.query.first()
        if legacy:
            return legacy.to_dict()
        return dict(cls.DEFAULT)

    @classmethod
    def save_config(cls, data):
        row = cls._relay_row()
        creds = row.raw_credentials() if row else {}
        creds['host'] = (data.get('host') or '').strip()
        creds['port'] = _coerce_port(data.get('port'))
        creds['username'] = (data.get('username') or '').strip()
        creds['use_tls'] = bool(data.get('use_tls', True))
        if data.get('provider_hint') is not None:
            creds['provider_hint'] = data.get('provider_hint')
        pw = data.get('password')
        if pw and not _is_masked(pw):
            creds['password'] = encrypt_secret(pw)
        enabled = bool(data.get('enabled', True))

        if row is None:
            row = EmailProviderConnection(
                provider='smtp', name=RELAY_PROVIDER_NAME,
                uses_relay=True, uses_notifications=False, relay_priority=0,
            )
            db.session.add(row)
        row.credentials_json = json.dumps(creds)
        row.is_active = enabled
        row.uses_relay = True
        db.session.commit()
        return {'config': cls._view_from_provider(row), 'apply': cls.apply()}

    @classmethod
    def apply(cls):
        """Push the active relay connection to Postfix (if installed)."""
        provider = EmailProviderService.relay_provider()  # active relay only
        status = PostfixService.get_status()
        if not status.get('installed'):
            return {'applied': False,
                    'note': 'Saved. Postfix is not installed here — the relay applies on a configured mail server.'}
        if provider:
            creds = provider.credentials()
            host = creds.get('host')
            if host:
                result = PostfixService.configure_relay(
                    host, _coerce_port(creds.get('port')), creds.get('username') or '',
                    creds.get('password') or '', _coerce_tls(creds.get('use_tls')))
                result['applied'] = result.get('success', False)
                return result
        result = PostfixService.disable_relay()
        result['applied'] = result.get('success', False)
        return result

    @classmethod
    def disable(cls):
        row = cls._relay_row()
        if row:
            row.is_active = False
            db.session.commit()
        return cls.apply()

    @classmethod
    def test(cls, data):
        """Open a real SMTP connection to validate host/port/credentials."""
        row = cls._relay_row()
        creds = row.credentials() if row else {}
        host = (data.get('host') or creds.get('host') or '').strip()
        if not host:
            return {'success': False, 'error': 'A relay host is required'}
        port = _coerce_port(data.get('port') or creds.get('port'))
        username = data.get('username') or creds.get('username') or ''
        use_tls = _coerce_tls(data.get('use_tls', creds.get('use_tls', True)))
        password = data.get('password')
        if password and _is_masked(password):
            password = None
        if not password:
            password = creds.get('password')  # already decrypted by credentials()

        try:
            if port == 465:
                server = smtplib.SMTP_SSL(host, port, timeout=15)
            else:
                server = smtplib.SMTP(host, port, timeout=15)
                server.ehlo()
                if use_tls:
                    server.starttls(context=ssl.create_default_context())
                    server.ehlo()
            if username:
                server.login(username, password or '')
            server.quit()
            return {'success': True, 'message': f'Connected to {host}:{port}'}
        except Exception as e:
            return {'success': False, 'error': str(e)}

    # ------------------------------------------------------------------ #
    # One-time legacy migration
    # ------------------------------------------------------------------ #
    @classmethod
    def migrate_legacy_config(cls):
        """Idempotently fold a legacy ``email_relay_config`` row into a
        ``uses_relay`` EmailProviderConnection. No-op if a relay connection
        already exists or there is no usable legacy row. The legacy password is
        already Fernet-encrypted with the same key, so it copies verbatim."""
        if cls._relay_row() is not None:
            return False
        legacy = EmailRelayConfig.query.first()
        if not legacy or not (legacy.host or '').strip():
            return False
        creds = {
            'host': legacy.host or '',
            'port': legacy.port or 587,
            'username': legacy.username or '',
            'use_tls': bool(legacy.use_tls),
        }
        if legacy.provider_hint:
            creds['provider_hint'] = legacy.provider_hint
        if legacy.password_encrypted:
            creds['password'] = legacy.password_encrypted  # already encrypted
        row = EmailProviderConnection(
            provider='smtp', name=RELAY_PROVIDER_NAME,
            credentials_json=json.dumps(creds),
            uses_relay=True, uses_notifications=False,
            is_active=bool(legacy.enabled), relay_priority=0,
        )
        db.session.add(row)
        db.session.commit()
        logger.info('Migrated legacy email relay config into EmailProviderConnection %s', row.id)
        return True
