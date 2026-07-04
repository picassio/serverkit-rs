"""§6 Phase C6b — EmailRelayService backed by the unified provider table.

The relay is an smtp EmailProviderConnection flagged uses_relay. Verifies
save/read/disable round-trips and the idempotent legacy migration. Postfix
apply is a no-op in tests (not installed), so we assert config state, not
Postfix side effects.
"""
from app.services.email_relay_service import EmailRelayService
from app.notifications.providers import EmailProviderService
from app.models.email_provider import EmailProviderConnection
from app.utils.crypto import decrypt_secret_safe


def test_save_creates_relay_provider(app):
    with app.app_context():
        EmailRelayService.save_config({
            'enabled': True, 'host': 'smtp.relay.io', 'port': 587,
            'username': 'relayuser', 'password': 'secretpw', 'use_tls': True,
            'provider_hint': 'mailgun',
        })
        view = EmailRelayService.get_config()
        assert view['enabled'] is True
        assert view['host'] == 'smtp.relay.io'
        assert view['username'] == 'relayuser'
        assert view['password_set'] is True
        assert view['provider_hint'] == 'mailgun'

        # Stored as a uses_relay smtp connection; password encrypted at rest.
        row = EmailProviderConnection.query.filter_by(uses_relay=True).first()
        assert row is not None and row.provider == 'smtp'
        assert row.uses_notifications is False
        raw_pw = row.raw_credentials().get('password')
        assert raw_pw and raw_pw != 'secretpw'
        assert decrypt_secret_safe(raw_pw) == 'secretpw'


def test_relay_provider_not_used_for_notifications(app):
    with app.app_context():
        EmailRelayService.save_config({
            'enabled': True, 'host': 'smtp.relay.io', 'username': 'u', 'password': 'p',
        })
        # The relay connection must not be picked as the notification default.
        assert EmailProviderService.default_provider() is None


def test_masked_password_preserved_on_resave(app):
    with app.app_context():
        EmailRelayService.save_config({'enabled': True, 'host': 'h', 'username': 'u', 'password': 'orig'})
        # Re-save with a masked password (UI round-trip) — secret must persist.
        EmailRelayService.save_config({'enabled': True, 'host': 'h2', 'username': 'u', 'password': '****'})
        row = EmailProviderConnection.query.filter_by(uses_relay=True).first()
        assert decrypt_secret_safe(row.raw_credentials().get('password')) == 'orig'
        assert EmailRelayService.get_config()['host'] == 'h2'


def test_disable_marks_inactive(app):
    with app.app_context():
        EmailRelayService.save_config({'enabled': True, 'host': 'h', 'username': 'u', 'password': 'p'})
        EmailRelayService.disable()
        assert EmailRelayService.get_config()['enabled'] is False
        # Inactive relay is not selected for applying.
        assert EmailProviderService.relay_provider() is None


def test_migrate_legacy_config_idempotent(app):
    from app import db
    from app.models.email import EmailRelayConfig
    from app.utils.crypto import encrypt_secret
    with app.app_context():
        legacy = EmailRelayConfig(
            enabled=True, host='legacy.smtp', port=2525, username='legacyuser',
            password_encrypted=encrypt_secret('legacypw'), use_tls=True, provider_hint='ses',
        )
        db.session.add(legacy)
        db.session.commit()

        assert EmailRelayService.migrate_legacy_config() is True
        row = EmailProviderConnection.query.filter_by(uses_relay=True).first()
        assert row is not None
        creds = row.credentials()
        assert creds['host'] == 'legacy.smtp'
        assert creds['password'] == 'legacypw'  # decrypts cleanly (same key)
        assert EmailRelayService.get_config()['host'] == 'legacy.smtp'

        # Idempotent: a second run is a no-op (relay row already exists).
        assert EmailRelayService.migrate_legacy_config() is False


def test_update_usage_smtp_only_relay(app):
    with app.app_context():
        sg = EmailProviderService.add_provider({'provider': 'sendgrid', 'name': 'SG', 'api_key': 'k'})
        EmailProviderService.update_usage(sg.id, {'uses_relay': True})
        assert EmailProviderConnection.query.get(sg.id).uses_relay is False  # API provider rejected

        smtp = EmailProviderService.add_provider({
            'provider': 'smtp', 'name': 'S', 'host': 'h', 'username': 'u', 'password': 'p'})
        EmailProviderService.update_usage(smtp.id, {'uses_relay': True, 'relay_priority': 3})
        updated = EmailProviderConnection.query.get(smtp.id)
        assert updated.uses_relay is True and updated.relay_priority == 3
