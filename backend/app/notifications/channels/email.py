"""Email channel.

Renders the event's Jinja template (HTML + text twin) and sends it. Transport
selection:

  1. a configured ``EmailProviderConnection`` (SMTP / SendGrid / Postmark / SES /
     Mailgun) if one exists — the Phase-5 path, with provider message-id capture;
  2. otherwise the legacy notifications.json SMTP config (back-compat).
"""
import logging
import smtplib
import ssl
from email.mime.multipart import MIMEMultipart
from email.mime.text import MIMEText

from app.notifications import catalog, rendering
from app.notifications.channels.base import ChannelAdapter, DeliveryResult

logger = logging.getLogger(__name__)


class EmailAdapter(ChannelAdapter):
    key = 'email'

    def _email_config(self):
        from app.services.notification_service import NotificationService
        return NotificationService.get_config().get('email', {})

    def _render(self, delivery, notification):
        data = notification.get_data()
        meta = catalog.resolve(
            notification.event_key, data,
            severity=notification.severity, title=notification.title,
        )
        recipient = {}
        if delivery.recipient is not None:
            recipient = {
                'name': delivery.recipient.username or delivery.recipient.email,
                'email': delivery.target,
            }
        return rendering.render_email(
            template=meta['template'],
            subject=notification.title,
            severity=notification.severity,
            data=data,
            recipient=recipient,
        )

    def deliver(self, delivery, notification):
        to_addr = (delivery.target or '').strip()
        if not to_addr:
            return DeliveryResult.skipped('no email target')

        rendered = self._render(delivery, notification)

        # 1) A configured provider connection wins.
        from app.notifications.providers import EmailProviderService
        provider = EmailProviderService.default_provider()
        if provider is not None:
            result = EmailProviderService.send(
                provider, to_addr, rendered['subject'], rendered['html'], rendered['text'])
            if result.get('success'):
                return DeliveryResult.sent(result.get('message_id'))
            return DeliveryResult.failed(result.get('error') or 'email send failed')

        # 2) Legacy notifications.json SMTP fallback.
        return self._send_legacy_smtp(to_addr, rendered)

    def _send_legacy_smtp(self, to_addr, rendered):
        config = self._email_config()
        if not config.get('enabled'):
            return DeliveryResult.skipped('no email provider configured')
        if not config.get('smtp_host') or not config.get('from_email'):
            return DeliveryResult.skipped('email channel not configured')

        msg = MIMEMultipart('alternative')
        msg['Subject'] = rendered['subject']
        msg['From'] = f"{config.get('from_name', 'ServerKit')} <{config['from_email']}>"
        msg['To'] = to_addr
        msg.attach(MIMEText(rendered['text'], 'plain', 'utf-8'))
        msg.attach(MIMEText(rendered['html'], 'html', 'utf-8'))

        host = config['smtp_host']
        port = int(config.get('smtp_port', 587))
        use_tls = config.get('smtp_tls', True)
        try:
            if use_tls:
                server = smtplib.SMTP(host, port, timeout=20)
                server.starttls(context=ssl.create_default_context())
            else:
                server = smtplib.SMTP_SSL(host, port, timeout=20, context=ssl.create_default_context())
            with server:
                if config.get('smtp_user') and config.get('smtp_password'):
                    server.login(config['smtp_user'], config['smtp_password'])
                server.send_message(msg)
        except smtplib.SMTPAuthenticationError as exc:
            return DeliveryResult.failed(f'SMTP auth failed: {exc}')
        except (smtplib.SMTPException, OSError) as exc:
            return DeliveryResult.failed(f'SMTP error: {exc}')
        return DeliveryResult.sent()
