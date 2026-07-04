"""Background consumer that delivers queued notifications.

Mirrors ``queue_bus/consumers/webhook_consumer.py``: a daemon thread polls the
``serverkit-system/notifications`` queue, loads each ``NotificationDelivery``,
hands it to the matching channel adapter, and maps the result back onto the
delivery row + the queue message. Retry, backoff, and dead-lettering are
inherited from the Queue Bus.
"""
import logging
import threading
import time
from datetime import datetime

from app import db
from app.notifications.channels import get_adapter
from app.notifications.channels.base import DeliveryResult
from app.notifications.models import NotificationDelivery
from app.notifications.service import GROUP_SLUG, QUEUE_SLUG, QUEUE_CONFIG, NotificationBusService
from app.queue_bus.service import QueueBusService
from app.services.telemetry_service import TelemetryService

logger = logging.getLogger(__name__)

_notification_consumer_thread = None


class NotificationConsumer:
    """Polls the queue bus for notification deliveries and transmits them."""

    def __init__(self, app=None):
        self.app = app
        self.running = False
        self.poll_interval_seconds = 1

    def start(self):
        if self.running:
            return
        self.running = True
        self._ensure_queue()
        thread = threading.Thread(target=self._run, daemon=True, name='queue-bus-notification-consumer')
        thread.start()
        logger.info('Notification queue consumer started')

    def stop(self):
        self.running = False

    def _ensure_queue(self):
        with self.app.app_context():
            QueueBusService.ensure_queue(GROUP_SLUG, QUEUE_SLUG, config=QUEUE_CONFIG)

    def _run(self):
        while self.running:
            try:
                with self.app.app_context():
                    self._process_batch()
            except Exception as exc:
                logger.error('Notification consumer error: %s', exc)
            time.sleep(self.poll_interval_seconds)

    def _process_batch(self):
        messages = QueueBusService.receive(
            GROUP_SLUG, QUEUE_SLUG, visibility_timeout_ms=60000, max_messages=10,
        )
        for message in messages:
            try:
                process_message(message)
            except Exception as exc:
                logger.error('Notification delivery failed for message %s: %s', message['id'], exc)
                try:
                    QueueBusService.fail(GROUP_SLUG, QUEUE_SLUG, message['id'], error_message=str(exc)[:500])
                except Exception as inner:  # pragma: no cover - defensive
                    logger.error('Failed to mark notification message failed: %s', inner)


def process_message(message):
    """Deliver one queued message. Shared by the consumer and tests."""
    payload = message.get('payload', {})
    delivery_id = payload.get('delivery_id')
    if not delivery_id:
        QueueBusService.complete(GROUP_SLUG, QUEUE_SLUG, message['id'])
        return

    delivery = NotificationDelivery.query.get(delivery_id)
    if not delivery:
        QueueBusService.complete(GROUP_SLUG, QUEUE_SLUG, message['id'])
        return

    # Idempotency: a duplicate/retried message for an already-sent delivery.
    if delivery.status == NotificationDelivery.STATUS_SENT:
        QueueBusService.complete(GROUP_SLUG, QUEUE_SLUG, message['id'])
        return

    adapter = get_adapter(delivery.channel)
    delivery.attempts = (delivery.attempts or 0) + 1

    # Unknown channel — don't retry forever; record and drop.
    if adapter is None:
        delivery.status = NotificationDelivery.STATUS_FAILED
        delivery.error = f'no adapter for channel {delivery.channel}'
        db.session.commit()
        QueueBusService.complete(GROUP_SLUG, QUEUE_SLUG, message['id'])
        return

    try:
        result = adapter.deliver(delivery, delivery.notification)
    except Exception as exc:
        delivery.error = str(exc)[:1000]
        db.session.commit()
        _fail(message, str(exc)[:500], delivery)
        return

    correlation_id = delivery.notification.correlation_id if delivery.notification else None

    if result.status == DeliveryResult.SENT:
        delivery.status = NotificationDelivery.STATUS_SENT
        delivery.sent_at = datetime.utcnow()
        delivery.provider_message_id = result.message_id
        delivery.error = None
        db.session.commit()
        _emit_delivery_telemetry(delivery, 'notification.delivered', correlation_id)
        QueueBusService.complete(GROUP_SLUG, QUEUE_SLUG, message['id'])
    elif result.status == DeliveryResult.SKIPPED:
        delivery.status = NotificationDelivery.STATUS_SKIPPED
        delivery.error = result.error
        db.session.commit()
        _emit_delivery_telemetry(delivery, 'notification.skipped', correlation_id, error=result.error)
        QueueBusService.complete(GROUP_SLUG, QUEUE_SLUG, message['id'])
    else:  # FAILED — let the queue retry / dead-letter
        delivery.error = result.error
        db.session.commit()
        _fail(message, result.error or 'delivery failed', delivery, correlation_id)


def _emit_delivery_telemetry(delivery, event_type, correlation_id, error=None):
    """Best-effort telemetry emission for a notification delivery outcome."""
    try:
        notification = delivery.notification
        severity = 'info'
        if event_type == 'notification.failed':
            severity = 'error'
        elif event_type == 'notification.skipped':
            severity = 'warning'
        elif notification and notification.severity in ('warning', 'error', 'critical'):
            severity = notification.severity

        TelemetryService.emit(
            source='notification',
            event_type=event_type,
            message=f'Notification {event_type.split(".")[-1]} via {delivery.channel}',
            severity=severity,
            correlation_id=correlation_id,
            payload={
                'notification_id': notification.id if notification else None,
                'delivery_id': delivery.id,
                'channel': delivery.channel,
                'target': delivery.target,
                'error': error,
            },
            commit=True,
        )
    except Exception:
        pass


def _fail(message, error_message, delivery, correlation_id=None):
    """Fail the queue message; if the queue dead-letters it, mark the delivery
    failed so the row reflects the terminal state."""
    res = QueueBusService.fail(GROUP_SLUG, QUEUE_SLUG, message['id'], error_message=error_message)
    if res and res.get('status') == 'dead_letter':
        delivery.status = NotificationDelivery.STATUS_FAILED
        db.session.commit()
        _emit_delivery_telemetry(delivery, 'notification.failed', correlation_id, error=error_message)
    else:
        db.session.commit()


def start_notification_consumer(app):
    """Start the singleton notification consumer thread.

    Skipped in testing mode (like the webhook consumer) to avoid background
    races against per-test databases.
    """
    global _notification_consumer_thread
    if _notification_consumer_thread is not None:
        return
    if app.config.get('ENV') == 'testing' or app.config.get('TESTING'):
        return
    consumer = NotificationConsumer(app)
    consumer.start()
    _notification_consumer_thread = consumer


def stop_notification_consumer():
    global _notification_consumer_thread
    if _notification_consumer_thread is not None:
        _notification_consumer_thread.stop()
        _notification_consumer_thread = None


def enqueue_notification_delivery(delivery_id, priority=0):
    """Enqueue a single delivery (parity helper; service.send does this itself)."""
    return NotificationBusService._enqueue(delivery_id, priority=priority)
