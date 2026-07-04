"""Plugin SDK for the ServerKit Queue Bus."""
from app.queue_bus.service import QueueBusService


class ReceivedMessage:
    """Handle returned by QueueBusSdk.receive() so callers can complete/fail."""

    def __init__(self, group_slug, queue_slug, message_dict):
        self.group_slug = group_slug
        self.queue_slug = queue_slug
        self.message_id = message_dict['id']
        self.payload = message_dict.get('payload')
        self._raw = message_dict

    def complete(self):
        return QueueBusService.complete(self.group_slug, self.queue_slug, self.message_id)

    def fail(self, error_message=None, requeue=False):
        return QueueBusService.fail(
            self.group_slug,
            self.queue_slug,
            self.message_id,
            error_message=error_message,
            requeue=requeue,
        )

    def to_dict(self):
        return self._raw


class QueueBusSdk:
    """Stable queue surface for plugins and internal consumers.

    Plugins do:

        from app.plugins_sdk import queue
        queue.send('git-server', 'sync-jobs', {'repo': 'foo/bar'})
    """

    def ensure(self, group_slug, queue_slug, config=None):
        """Ensure the group and queue exist."""
        return QueueBusService.ensure_queue(group_slug, queue_slug, config=config)

    def send(self, group_slug, queue_slug, payload, priority=0, delay_ms=0, max_attempts=None, ensure=True):
        """Send a message to a queue."""
        if ensure:
            self.ensure(group_slug, queue_slug)
        return QueueBusService.send(
            group_slug=group_slug,
            queue_slug=queue_slug,
            payload=payload,
            priority=priority,
            delay_ms=delay_ms,
            max_attempts=max_attempts,
        )

    def receive(self, group_slug, queue_slug, visibility_timeout_ms=30000, ensure=True):
        """Receive a single message. Returns ReceivedMessage or None."""
        if ensure:
            self.ensure(group_slug, queue_slug)
        messages = QueueBusService.receive(
            group_slug=group_slug,
            queue_slug=queue_slug,
            visibility_timeout_ms=visibility_timeout_ms,
            max_messages=1,
        )
        if not messages:
            return None
        return ReceivedMessage(group_slug, queue_slug, messages[0])

    def receive_many(self, group_slug, queue_slug, visibility_timeout_ms=30000, max_messages=10, ensure=True):
        """Receive multiple messages. Returns list of ReceivedMessage."""
        if ensure:
            self.ensure(group_slug, queue_slug)
        messages = QueueBusService.receive(
            group_slug=group_slug,
            queue_slug=queue_slug,
            visibility_timeout_ms=visibility_timeout_ms,
            max_messages=max_messages,
        )
        return [ReceivedMessage(group_slug, queue_slug, m) for m in messages]

    def stats(self, group_slug=None, queue_slug=None):
        return QueueBusService.get_stats(group_slug, queue_slug)
