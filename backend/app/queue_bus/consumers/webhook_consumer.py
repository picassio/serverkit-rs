"""Internal consumer that delivers webhooks via the Queue Bus."""
import hashlib
import hmac
import json
import logging
import threading
import time
import uuid
from datetime import datetime

import requests as http_requests

from app import db
from app.queue_bus.service import QueueBusService
from app.models.event_subscription import EventSubscription, EventDelivery
from app.services.telemetry_service import TelemetryService, generate_correlation_id

logger = logging.getLogger(__name__)

GROUP_SLUG = 'serverkit-system'
QUEUE_SLUG = 'webhook-deliveries'

_webhook_consumer_thread = None


class WebhookConsumer:
    """Polls the queue bus for webhook delivery messages and executes them."""

    def __init__(self, app=None):
        self.app = app
        self.running = False
        self.poll_interval_seconds = 1

    def start(self):
        if self.running:
            return
        self.running = True
        self._ensure_queue()
        thread = threading.Thread(target=self._run, daemon=True, name='queue-bus-webhook-consumer')
        thread.start()
        logger.info('Webhook queue consumer started')

    def stop(self):
        self.running = False

    def _ensure_queue(self):
        with self.app.app_context():
            QueueBusService.ensure_queue(
                GROUP_SLUG,
                QUEUE_SLUG,
                config={'max_attempts': 3, 'visibility_timeout_ms': 60000},
            )

    def _run(self):
        while self.running:
            try:
                with self.app.app_context():
                    self._process_batch()
            except Exception as e:
                logger.error(f'Webhook consumer error: {e}')
            time.sleep(self.poll_interval_seconds)

    def _process_batch(self):
        messages = QueueBusService.receive(
            GROUP_SLUG,
            QUEUE_SLUG,
            visibility_timeout_ms=60000,
            max_messages=10,
        )
        for message in messages:
            try:
                self._deliver(message)
            except Exception as e:
                logger.error(f'Webhook delivery failed for message {message["id"]}: {e}')
                try:
                    QueueBusService.fail(
                        GROUP_SLUG,
                        QUEUE_SLUG,
                        message['id'],
                        error_message=str(e)[:500],
                    )
                except Exception as inner:
                    logger.error(f'Failed to mark webhook message failed: {inner}')

    def _deliver(self, message):
        payload = message.get('payload', {})
        delivery_id = payload.get('delivery_id')
        if not delivery_id:
            QueueBusService.complete(GROUP_SLUG, QUEUE_SLUG, message['id'])
            return

        delivery = EventDelivery.query.get(delivery_id)
        if not delivery:
            QueueBusService.complete(GROUP_SLUG, QUEUE_SLUG, message['id'])
            return

        subscription = delivery.subscription
        correlation_id = delivery.correlation_id
        if not subscription or not subscription.is_active:
            delivery.status = EventDelivery.STATUS_FAILED
            db.session.commit()
            _emit_webhook_telemetry(delivery, 'webhook.failed', correlation_id,
                                    error='subscription inactive or missing')
            QueueBusService.complete(GROUP_SLUG, QUEUE_SLUG, message['id'])
            return

        event_payload = delivery.get_payload()
        payload_json = json.dumps(event_payload)
        delivery_uuid = str(uuid.uuid4())

        headers = {
            'Content-Type': 'application/json',
            'X-ServerKit-Event': delivery.event_type,
            'X-ServerKit-Delivery': delivery_uuid,
            'User-Agent': 'ServerKit-Webhooks/1.0',
        }

        if subscription.secret:
            signature = hmac.new(
                subscription.secret.encode(),
                payload_json.encode(),
                hashlib.sha256,
            ).hexdigest()
            headers['X-ServerKit-Signature'] = f'sha256={signature}'

        custom_headers = subscription.get_headers()
        if custom_headers:
            headers.update(custom_headers)

        delivery.attempts = (delivery.attempts or 0) + 1
        start_time = time.time()

        try:
            resp = http_requests.post(
                subscription.url,
                data=payload_json,
                headers=headers,
                timeout=subscription.timeout_seconds or 10,
            )
            elapsed_ms = (time.time() - start_time) * 1000
            delivery.http_status = resp.status_code
            delivery.response_body = resp.text[:1000] if resp.text else None
            delivery.duration_ms = round(elapsed_ms, 2)

            if 200 <= resp.status_code < 300:
                delivery.status = EventDelivery.STATUS_SUCCESS
                delivery.delivered_at = datetime.utcnow()
                db.session.commit()
                _emit_webhook_telemetry(delivery, 'webhook.delivered', correlation_id)
                QueueBusService.complete(GROUP_SLUG, QUEUE_SLUG, message['id'])
            else:
                db.session.commit()
                _emit_webhook_telemetry(delivery, 'webhook.failed', correlation_id,
                                        error=f'HTTP {resp.status_code}')
                QueueBusService.fail(
                    GROUP_SLUG,
                    QUEUE_SLUG,
                    message['id'],
                    error_message=f'HTTP {resp.status_code}',
                )

        except Exception as e:
            elapsed_ms = (time.time() - start_time) * 1000
            delivery.duration_ms = round(elapsed_ms, 2)
            delivery.response_body = str(e)[:1000]
            db.session.commit()
            _emit_webhook_telemetry(delivery, 'webhook.failed', correlation_id,
                                    error=str(e)[:500])
            QueueBusService.fail(
                GROUP_SLUG,
                QUEUE_SLUG,
                message['id'],
                error_message=str(e)[:500],
            )


def _emit_webhook_telemetry(delivery, event_type, correlation_id, error=None):
    """Best-effort telemetry emission for a webhook delivery outcome."""
    try:
        TelemetryService.emit(
            source='webhook',
            event_type=event_type,
            message=f'Webhook {event_type.split(".")[-1]}: {delivery.event_type}',
            severity='error' if event_type == 'webhook.failed' else 'info',
            correlation_id=correlation_id,
            payload={
                'delivery_id': delivery.id,
                'subscription_id': delivery.subscription_id,
                'event_type': delivery.event_type,
                'http_status': delivery.http_status,
                'duration_ms': delivery.duration_ms,
                'error': error,
            },
            commit=True,
        )
    except Exception:
        pass


def start_webhook_consumer(app):
    """Start the singleton webhook consumer thread.

    Skipped in testing mode to avoid background noise and race conditions
    against per-test databases.
    """
    global _webhook_consumer_thread
    if _webhook_consumer_thread is not None:
        return
    if app.config.get('ENV') == 'testing' or app.config.get('TESTING'):
        return
    consumer = WebhookConsumer(app)
    consumer.start()
    _webhook_consumer_thread = consumer


def stop_webhook_consumer():
    global _webhook_consumer_thread
    if _webhook_consumer_thread is not None:
        _webhook_consumer_thread.stop()
        _webhook_consumer_thread = None


def enqueue_webhook_delivery(delivery_id):
    """Enqueue a webhook delivery message on the queue bus."""
    QueueBusService.ensure_queue(GROUP_SLUG, QUEUE_SLUG)
    return QueueBusService.send(
        GROUP_SLUG,
        QUEUE_SLUG,
        {'delivery_id': delivery_id},
    )
