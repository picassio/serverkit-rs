"""High-level service for the ServerKit Queue Bus."""
import logging

from app.queue_bus.sqlalchemy_broker import SQLAlchemyBroker, QueueBusError
from app.utils.slug import unique_slug

logger = logging.getLogger(__name__)


class QueueBusService:
    """Thin wrapper around the configured broker.

    The service is the single entry point used by API routes, internal
    consumers, and the plugin SDK. Today it always uses SQLAlchemyBroker;
    in the future it can select a broker from config.
    """

    _broker = None

    @classmethod
    def broker(cls):
        if cls._broker is None:
            cls._broker = SQLAlchemyBroker()
        return cls._broker

    @classmethod
    def reset_broker(cls):
        """Reset the broker singleton. Useful for tests."""
        cls._broker = None

    # ------------------------------------------------------------------
    # Groups
    # ------------------------------------------------------------------
    @classmethod
    def create_group(cls, slug=None, name=None, description=None, owner_type='system', owner_id=None, config=None):
        base = slug or name
        if not base:
            raise QueueBusError('Either slug or name is required', 400)
        generated_slug = unique_slug(base, lambda s: cls.broker().get_group(s) is not None)
        return cls.broker().create_group(
            slug=generated_slug,
            name=name or generated_slug,
            description=description,
            owner_type=owner_type,
            owner_id=owner_id,
            config=config,
        )

    @classmethod
    def update_group(cls, slug, name=None, description=None, config=None):
        return cls.broker().update_group(slug, name, description, config)

    @classmethod
    def delete_group(cls, slug):
        return cls.broker().delete_group(slug)

    @classmethod
    def get_group(cls, slug):
        return cls.broker().get_group(slug)

    @classmethod
    def list_groups(cls, owner_type=None, owner_id=None, limit=100, offset=0):
        return cls.broker().list_groups(owner_type, owner_id, limit, offset)

    # ------------------------------------------------------------------
    # Queues
    # ------------------------------------------------------------------
    @classmethod
    def create_queue(cls, group_slug, slug=None, name=None, description=None, config=None):
        base = slug or name
        if not base:
            raise QueueBusError('Either slug or name is required', 400)
        generated_slug = unique_slug(base, lambda s: cls.broker().get_queue(group_slug, s) is not None)
        return cls.broker().create_queue(
            group_slug=group_slug,
            slug=generated_slug,
            name=name or generated_slug,
            description=description,
            config=config,
        )

    @classmethod
    def update_queue(cls, group_slug, slug, name=None, description=None, config=None):
        return cls.broker().update_queue(group_slug, slug, name, description, config)

    @classmethod
    def delete_queue(cls, group_slug, queue_slug):
        return cls.broker().delete_queue(group_slug, queue_slug)

    @classmethod
    def get_queue(cls, group_slug, queue_slug):
        return cls.broker().get_queue(group_slug, queue_slug)

    @classmethod
    def list_queues(cls, group_slug, limit=100, offset=0):
        return cls.broker().list_queues(group_slug, limit, offset)

    # ------------------------------------------------------------------
    # Messages
    # ------------------------------------------------------------------
    @classmethod
    def send(cls, group_slug, queue_slug, payload, priority=0, delay_ms=0, max_attempts=None):
        return cls.broker().send(
            group_slug=group_slug,
            queue_slug=queue_slug,
            payload=payload,
            priority=priority,
            delay_ms=delay_ms,
            max_attempts=max_attempts,
        )

    @classmethod
    def receive(cls, group_slug, queue_slug, visibility_timeout_ms=30000, max_messages=1):
        return cls.broker().receive(
            group_slug=group_slug,
            queue_slug=queue_slug,
            visibility_timeout_ms=visibility_timeout_ms,
            max_messages=max_messages,
        )

    @classmethod
    def complete(cls, group_slug, queue_slug, message_id):
        return cls.broker().complete(group_slug, queue_slug, message_id)

    @classmethod
    def fail(cls, group_slug, queue_slug, message_id, error_message=None, requeue=False):
        return cls.broker().fail(
            group_slug=group_slug,
            queue_slug=queue_slug,
            message_id=message_id,
            error_message=error_message,
            requeue=requeue,
        )

    @classmethod
    def requeue(cls, group_slug, queue_slug, message_id):
        return cls.broker().requeue(group_slug, queue_slug, message_id)

    @classmethod
    def delete_message(cls, group_slug, queue_slug, message_id):
        return cls.broker().delete_message(group_slug, queue_slug, message_id)

    @classmethod
    def get_message(cls, group_slug, queue_slug, message_id):
        return cls.broker().get_message(group_slug, queue_slug, message_id)

    @classmethod
    def list_messages(cls, group_slug, queue_slug, status=None, limit=100, offset=0):
        return cls.broker().list_messages(
            group_slug=group_slug,
            queue_slug=queue_slug,
            status=status,
            limit=limit,
            offset=offset,
        )

    @classmethod
    def get_stats(cls, group_slug=None, queue_slug=None):
        return cls.broker().get_stats(group_slug, queue_slug)

    @classmethod
    def ensure_queue(cls, group_slug, queue_slug, config=None):
        """Idempotently ensure a group and queue exist. Used by SDK/consumers."""
        group = cls.get_group(group_slug)
        if not group:
            cls.create_group(group_slug, name=group_slug.replace('-', ' ').title())
        queue = cls.get_queue(group_slug, queue_slug)
        if not queue:
            return cls.create_queue(group_slug, queue_slug, name=queue_slug.replace('-', ' ').title(), config=config)
        return queue
