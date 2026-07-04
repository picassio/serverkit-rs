"""Abstract broker interface for the ServerKit Queue Bus."""
from abc import ABC, abstractmethod


class AbstractBroker(ABC):
    """Pluggable backend for the queue bus.

    Implementations must be stateless with respect to in-memory state so that
    multiple worker processes can share the same logical queue.
    """

    @abstractmethod
    def create_group(self, slug, name, description=None, owner_type='system', owner_id=None, config=None):
        """Create a QueueGroup. Return a dict or raise QueueBusError."""

    @abstractmethod
    def update_group(self, slug, name=None, description=None, config=None):
        """Update a QueueGroup."""

    @abstractmethod
    def delete_group(self, slug):
        """Delete a QueueGroup and all its queues/messages."""

    @abstractmethod
    def get_group(self, slug):
        """Return a QueueGroup dict or None."""

    @abstractmethod
    def list_groups(self, owner_type=None, owner_id=None, limit=100, offset=0):
        """Return a list of QueueGroup dicts."""

    @abstractmethod
    def create_queue(self, group_slug, slug, name, description=None, config=None):
        """Create a Queue inside a group."""

    @abstractmethod
    def update_queue(self, group_slug, slug, name=None, description=None, config=None):
        """Update a Queue."""

    @abstractmethod
    def delete_queue(self, group_slug, queue_slug):
        """Delete a Queue and all its messages."""

    @abstractmethod
    def get_queue(self, group_slug, queue_slug):
        """Return a Queue dict or None."""

    @abstractmethod
    def list_queues(self, group_slug, limit=100, offset=0):
        """Return a list of Queue dicts."""

    @abstractmethod
    def send(self, group_slug, queue_slug, payload, priority=0, delay_ms=0, max_attempts=None):
        """Send a message to a queue. Return message id."""

    @abstractmethod
    def receive(self, group_slug, queue_slug, visibility_timeout_ms=30000, max_messages=1):
        """Receive up to max_messages pending messages. Return list of message dicts."""

    @abstractmethod
    def complete(self, group_slug, queue_slug, message_id):
        """Mark a message as completed."""

    @abstractmethod
    def fail(self, group_slug, queue_slug, message_id, error_message=None, requeue=False):
        """Mark a message failed. If requeue is False, retry or dead-letter based on attempts."""

    @abstractmethod
    def requeue(self, group_slug, queue_slug, message_id):
        """Move a failed/dead-letter message back to pending."""

    @abstractmethod
    def delete_message(self, group_slug, queue_slug, message_id):
        """Permanently delete a message."""

    @abstractmethod
    def get_message(self, group_slug, queue_slug, message_id):
        """Return a message dict or None."""

    @abstractmethod
    def list_messages(self, group_slug, queue_slug, status=None, limit=100, offset=0):
        """Return a list of message dicts."""

    @abstractmethod
    def get_stats(self, group_slug=None, queue_slug=None):
        """Return aggregated stats."""
