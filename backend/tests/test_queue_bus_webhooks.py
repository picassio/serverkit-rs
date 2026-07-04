"""Tests for webhook delivery integration with the Queue Bus."""
from app import db
from app.models.user import User
from app.models.event_subscription import EventSubscription, EventDelivery
from app.services.event_service import EventService
from app.queue_bus.service import QueueBusService
from app.queue_bus.models import QueueGroup


def _seed_subscription(user_id, url='https://example.com/webhook', events=None):
    sub = EventSubscription(
        user_id=user_id,
        name='Test Webhook',
        url=url,
    )
    sub.set_events(events or ['app.created'])
    db.session.add(sub)
    db.session.commit()
    return sub


class TestWebhookQueueIntegration:
    def test_emit_enqueues_webhook_delivery(self, app, auth_headers):
        from app.models import User
        user = User.query.filter_by(username='testadmin').first()
        sub = _seed_subscription(user.id)

        EventService.emit('app.created', {'app_id': 1, 'name': 'test'})

        delivery = EventDelivery.query.filter_by(subscription_id=sub.id).first()
        assert delivery is not None
        assert delivery.status == EventDelivery.STATUS_PENDING

        # A message should exist on the system webhook queue.
        QueueBusService.ensure_queue('serverkit-system', 'webhook-deliveries')
        messages = QueueBusService.receive('serverkit-system', 'webhook-deliveries')
        assert len(messages) == 1
        assert messages[0]['payload']['delivery_id'] == delivery.id

    def test_emit_no_subscription_no_message(self, app, auth_headers):
        EventService.emit('app.created', {'app_id': 1})
        QueueBusService.ensure_queue('serverkit-system', 'webhook-deliveries')
        messages = QueueBusService.receive('serverkit-system', 'webhook-deliveries')
        assert messages == []

    def test_webhook_consumer_delivers_success(self, app, monkeypatch, auth_headers):
        from app.models import User
        from app.queue_bus.consumers.webhook_consumer import WebhookConsumer

        user = User.query.filter_by(username='testadmin').first()
        sub = _seed_subscription(user.id)
        EventService.emit('app.created', {'app_id': 1})
        delivery = EventDelivery.query.filter_by(subscription_id=sub.id).first()

        calls = []

        def fake_post(url, data, headers, timeout):
            class Resp:
                status_code = 200
                text = 'ok'
            calls.append((url, data, headers, timeout))
            return Resp()

        monkeypatch.setattr('app.queue_bus.consumers.webhook_consumer.http_requests.post', fake_post)

        consumer = WebhookConsumer(app)
        consumer._process_batch()

        assert len(calls) == 1
        delivery_after = EventDelivery.query.get(delivery.id)
        assert delivery_after.status == EventDelivery.STATUS_SUCCESS

    def test_webhook_consumer_retries_then_dead_letters(self, app, monkeypatch, auth_headers):
        from app.models import User
        from app.queue_bus.consumers.webhook_consumer import WebhookConsumer

        user = User.query.filter_by(username='testadmin').first()
        sub = _seed_subscription(user.id)
        EventService.emit('app.created', {'app_id': 1})
        delivery = EventDelivery.query.filter_by(subscription_id=sub.id).first()

        QueueBusService.ensure_queue('serverkit-system', 'webhook-deliveries')
        from app.queue_bus.models import Queue
        queue = Queue.query.join(Queue.group).filter(
            QueueGroup.slug == 'serverkit-system',
            Queue.slug == 'webhook-deliveries',
        ).first()

        def fake_post(*args, **kwargs):
            class Resp:
                status_code = 500
                text = 'error'
            return Resp()

        monkeypatch.setattr('app.queue_bus.consumers.webhook_consumer.http_requests.post', fake_post)

        consumer = WebhookConsumer(app)
        # Three attempts: initial + 2 retries. Bypass the bus retry delay in tests
        # by resetting visible_after between iterations.
        from app.queue_bus.models import QueueMessage
        from datetime import datetime
        for _ in range(3):
            consumer._process_batch()
            msg = QueueMessage.query.filter_by(queue_id=queue.id).first()
            if msg and msg.status == QueueMessage.STATUS_PENDING:
                msg.visible_after = datetime.utcnow()
                db.session.commit()

        delivery_after = EventDelivery.query.get(delivery.id)
        assert delivery_after.attempts == 3
        # The bus message should be dead-lettered after max_attempts.
        QueueBusService.ensure_queue('serverkit-system', 'webhook-deliveries')
        pending = QueueBusService.receive('serverkit-system', 'webhook-deliveries')
        assert pending == []
