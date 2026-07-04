"""Chat / webhook channels (Discord, Slack, Telegram, generic webhook).

These delegate to the existing, working ``NotificationService`` formatters so we
don't re-implement Discord embeds / Slack blocks / Telegram HTML. The bus sends
to the *per-recipient* target (a user's personal Discord webhook or Telegram
chat id), reusing only shared secrets (e.g. the system Telegram bot token) from
the saved config. System-wide chat broadcast stays on the legacy path until
Phase 6.
"""
import logging

from app.notifications.channels.base import ChannelAdapter, DeliveryResult

logger = logging.getLogger(__name__)


class ChatAdapter(ChannelAdapter):
    """One adapter class, instantiated per channel key."""

    def __init__(self, key):
        self.key = key

    def _to_alert(self, notification):
        data = notification.get_data()
        return {
            'severity': notification.severity or 'info',
            'type': notification.event_key,
            'message': data.get('message') or notification.body or notification.title,
        }

    def _alerts_for(self, notification):
        """Use the original multi-alert list for system broadcasts (so chat keeps
        per-alert formatting); otherwise a single synthesized alert."""
        alerts = notification.get_data().get('alerts')
        if isinstance(alerts, list) and alerts:
            return alerts
        return [self._to_alert(notification)]

    def deliver(self, delivery, notification):
        target = (delivery.target or '').strip()
        if not target:
            return DeliveryResult.skipped(f'no {self.key} target')

        from app.services.notification_service import NotificationService
        saved = NotificationService.get_config()
        sev = notification.severity or 'info'
        alerts = self._alerts_for(notification)
        # Let every alert through regardless of the saved channel's notify_on
        # (gating already happened upstream in the bus).
        notify_on = sorted({a.get('severity', sev) for a in alerts} | {sev})

        if self.key == 'discord':
            cfg = {**saved.get('discord', {}), 'enabled': True,
                   'webhook_url': target, 'notify_on': notify_on}
            result = NotificationService.send_discord(alerts, cfg)
        elif self.key == 'slack':
            cfg = {**saved.get('slack', {}), 'enabled': True,
                   'webhook_url': target, 'notify_on': notify_on}
            result = NotificationService.send_slack(alerts, cfg)
        elif self.key == 'telegram':
            cfg = {**saved.get('telegram', {}), 'enabled': True,
                   'chat_id': target, 'notify_on': notify_on}
            result = NotificationService.send_telegram(alerts, cfg)
        elif self.key == 'webhook':
            cfg = {**saved.get('generic_webhook', {}), 'enabled': True,
                   'url': target, 'notify_on': notify_on}
            result = NotificationService.send_generic_webhook(alerts, cfg)
        else:
            return DeliveryResult.skipped(f'unsupported chat channel {self.key}')

        if result.get('success'):
            return DeliveryResult.sent()
        return DeliveryResult.failed(result.get('error') or 'send failed')
