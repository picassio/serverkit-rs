"""§6 Phase C6a — email provider usage flags.

One EmailProviderConnection can drive the Notification Bus and/or the Postfix
relay. Notification selection ignores relay-only rows; a relay selector picks
the flagged SMTP connection by priority.
"""
from app.models.email_provider import EmailProviderConnection
from app.notifications.providers import EmailProviderService


def test_usage_flags_default_and_serialize(app):
    with app.app_context():
        p = EmailProviderService.add_provider({
            'provider': 'smtp', 'name': 'Main SMTP',
            'host': 'smtp.example.com', 'port': 587, 'username': 'u', 'password': 'pw',
        })
        d = p.to_dict()
        assert d['uses_notifications'] is True
        assert d['uses_relay'] is False
        assert d['relay_priority'] == 0


def test_default_provider_ignores_relay_only(app):
    with app.app_context():
        # A relay-only row must never be chosen for notifications.
        EmailProviderService.add_provider({
            'provider': 'smtp', 'name': 'Relay only',
            'host': 'relay.example.com', 'port': 587, 'username': 'u', 'password': 'pw',
            'uses_notifications': False, 'uses_relay': True, 'is_default': True,
        })
        assert EmailProviderService.default_provider() is None

        notif = EmailProviderService.add_provider({
            'provider': 'sendgrid', 'name': 'SG', 'api_key': 'sg-key',
        })
        chosen = EmailProviderService.default_provider()
        assert chosen is not None and chosen.id == notif.id


def test_relay_provider_selection_by_priority(app):
    with app.app_context():
        low = EmailProviderService.add_provider({
            'provider': 'smtp', 'name': 'low', 'host': 'a', 'username': 'u', 'password': 'p',
            'uses_relay': True, 'relay_priority': 1, 'uses_notifications': False,
        })
        high = EmailProviderService.add_provider({
            'provider': 'smtp', 'name': 'high', 'host': 'b', 'username': 'u', 'password': 'p',
            'uses_relay': True, 'relay_priority': 5, 'uses_notifications': False,
        })
        chosen = EmailProviderService.relay_provider()
        assert chosen.id == high.id
        assert low.id != high.id


def test_api_provider_cannot_be_relay(app):
    with app.app_context():
        # uses_relay requested on a non-SMTP provider is ignored.
        p = EmailProviderService.add_provider({
            'provider': 'sendgrid', 'name': 'SG', 'api_key': 'k', 'uses_relay': True,
        })
        assert p.uses_relay is False
