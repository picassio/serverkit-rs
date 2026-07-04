"""SQLAlchemy-backed broker for the ServerKit Queue Bus."""
import logging
from datetime import datetime, timedelta

from sqlalchemy import func

from app import db
from app.queue_bus.broker import AbstractBroker
from app.queue_bus.models import QueueGroup, Queue, QueueMessage

logger = logging.getLogger(__name__)


class QueueBusError(Exception):
    """Domain error for queue operations."""

    def __init__(self, message, status_code=400):
        super().__init__(message)
        self.message = message
        self.status_code = status_code


class SQLAlchemyBroker(AbstractBroker):
    """Default broker implementation using the existing SQLAlchemy database."""

    # ------------------------------------------------------------------
    # Groups
    # ------------------------------------------------------------------
    def create_group(self, slug, name, description=None, owner_type='system', owner_id=None, config=None):
        if not slug or not _valid_slug(slug):
            raise QueueBusError('Invalid group slug', 400)
        if QueueGroup.query.filter_by(slug=slug).first():
            raise QueueBusError('Group already exists', 409)

        group = QueueGroup(
            slug=slug,
            name=name or slug,
            description=description,
            owner_type=owner_type,
            owner_id=owner_id,
        )
        group.set_config(config or {})
        db.session.add(group)
        db.session.commit()
        return group.to_dict(include_stats=True)

    def update_group(self, slug, name=None, description=None, config=None):
        group = QueueGroup.query.filter_by(slug=slug).first()
        if not group:
            raise QueueBusError('Group not found', 404)
        if name is not None:
            group.name = name
        if description is not None:
            group.description = description
        if config is not None:
            group.set_config(config)
        db.session.commit()
        return group.to_dict(include_stats=True)

    def delete_group(self, slug):
        group = QueueGroup.query.filter_by(slug=slug).first()
        if not group:
            raise QueueBusError('Group not found', 404)
        db.session.delete(group)
        db.session.commit()
        return {'success': True}

    def get_group(self, slug):
        group = QueueGroup.query.filter_by(slug=slug).first()
        if not group:
            return None
        return group.to_dict(include_stats=True)

    def list_groups(self, owner_type=None, owner_id=None, limit=100, offset=0):
        query = QueueGroup.query
        if owner_type:
            query = query.filter_by(owner_type=owner_type)
        if owner_id:
            query = query.filter_by(owner_id=owner_id)
        groups = query.order_by(QueueGroup.created_at.desc()).limit(limit).offset(offset).all()
        return [g.to_dict(include_stats=True) for g in groups]

    # ------------------------------------------------------------------
    # Queues
    # ------------------------------------------------------------------
    def create_queue(self, group_slug, slug, name, description=None, config=None):
        if not slug or not _valid_slug(slug):
            raise QueueBusError('Invalid queue slug', 400)
        group = QueueGroup.query.filter_by(slug=group_slug).first()
        if not group:
            raise QueueBusError('Group not found', 404)
        if Queue.query.filter_by(group_id=group.id, slug=slug).first():
            raise QueueBusError('Queue already exists in group', 409)

        queue = Queue(
            group_id=group.id,
            slug=slug,
            name=name or slug,
            description=description,
        )
        queue.set_config(config or {})
        db.session.add(queue)
        db.session.commit()
        return queue.to_dict(include_stats=True)

    def update_queue(self, group_slug, slug, name=None, description=None, config=None):
        queue = self._get_queue_or_raise(group_slug, slug)
        if name is not None:
            queue.name = name
        if description is not None:
            queue.description = description
        if config is not None:
            queue.set_config(config)
        db.session.commit()
        return queue.to_dict(include_stats=True)

    def delete_queue(self, group_slug, queue_slug):
        queue = self._get_queue_or_raise(group_slug, queue_slug)
        db.session.delete(queue)
        db.session.commit()
        return {'success': True}

    def get_queue(self, group_slug, queue_slug):
        queue = self._get_queue(group_slug, queue_slug)
        if not queue:
            return None
        return queue.to_dict(include_stats=True)

    def list_queues(self, group_slug, limit=100, offset=0):
        group = QueueGroup.query.filter_by(slug=group_slug).first()
        if not group:
            raise QueueBusError('Group not found', 404)
        queues = Queue.query.filter_by(group_id=group.id).order_by(Queue.created_at.desc()).limit(limit).offset(offset).all()
        return [q.to_dict(include_stats=True) for q in queues]

    # ------------------------------------------------------------------
    # Messages
    # ------------------------------------------------------------------
    def send(self, group_slug, queue_slug, payload, priority=0, delay_ms=0, max_attempts=None):
        queue = self._get_queue_or_raise(group_slug, queue_slug)
        visible_after = datetime.utcnow()
        if delay_ms:
            visible_after += timedelta(milliseconds=delay_ms)

        queue_config = queue.get_config()
        message_max_attempts = max_attempts or queue_config.get('max_attempts', 3)

        message = QueueMessage(
            queue_id=queue.id,
            group_id=queue.group_id,
            status=QueueMessage.STATUS_PENDING,
            priority=priority,
            max_attempts=message_max_attempts,
            visible_after=visible_after,
        )
        message.set_payload(payload)
        db.session.add(message)
        db.session.commit()
        return message.to_dict()

    def receive(self, group_slug, queue_slug, visibility_timeout_ms=30000, max_messages=1):
        queue = self._get_queue_or_raise(group_slug, queue_slug)
        now = datetime.utcnow()
        invisible_until = now + timedelta(milliseconds=visibility_timeout_ms)

        # Pending messages that are visible (no delay) and not currently invisible.
        messages = QueueMessage.query.filter(
            QueueMessage.queue_id == queue.id,
            QueueMessage.status == QueueMessage.STATUS_PENDING,
            QueueMessage.visible_after <= now,
            db.or_(
                QueueMessage.invisible_until.is_(None),
                QueueMessage.invisible_until <= now,
            ),
        ).order_by(
            QueueMessage.priority.desc(),
            QueueMessage.created_at.asc(),
        ).limit(max_messages).with_for_update().all()

        for message in messages:
            message.status = QueueMessage.STATUS_IN_FLIGHT
            message.invisible_until = invisible_until
            message.attempts += 1

        db.session.commit()
        return [m.to_dict() for m in messages]

    def complete(self, group_slug, queue_slug, message_id):
        message = self._get_message_or_raise(group_slug, queue_slug, message_id)
        message.status = QueueMessage.STATUS_COMPLETED
        message.completed_at = datetime.utcnow()
        message.invisible_until = None
        db.session.commit()
        return message.to_dict()

    def fail(self, group_slug, queue_slug, message_id, error_message=None, requeue=False):
        message = self._get_message_or_raise(group_slug, queue_slug, message_id)
        message.error_message = (message.error_message or '') + f'[{datetime.utcnow().isoformat()}] {error_message}\n' if error_message else message.error_message

        if requeue:
            message.status = QueueMessage.STATUS_PENDING
            message.invisible_until = None
            db.session.commit()
            return message.to_dict()

        if message.attempts >= message.max_attempts:
            message.status = QueueMessage.STATUS_DEAD_LETTER
            message.invisible_until = None
        else:
            # Exponential backoff: 10s, 30s, 90s, ...
            delay_seconds = 10 * (3 ** (message.attempts - 1))
            message.status = QueueMessage.STATUS_PENDING
            message.visible_after = datetime.utcnow() + timedelta(seconds=delay_seconds)
            message.invisible_until = None

        db.session.commit()
        return message.to_dict()

    def requeue(self, group_slug, queue_slug, message_id):
        message = self._get_message_or_raise(group_slug, queue_slug, message_id)
        if message.status not in (QueueMessage.STATUS_FAILED, QueueMessage.STATUS_DEAD_LETTER):
            raise QueueBusError('Only failed or dead-letter messages can be requeued', 400)
        message.status = QueueMessage.STATUS_PENDING
        message.visible_after = datetime.utcnow()
        message.invisible_until = None
        db.session.commit()
        return message.to_dict()

    def delete_message(self, group_slug, queue_slug, message_id):
        message = self._get_message_or_raise(group_slug, queue_slug, message_id)
        db.session.delete(message)
        db.session.commit()
        return {'success': True}

    def get_message(self, group_slug, queue_slug, message_id):
        message = self._get_message(group_slug, queue_slug, message_id)
        if not message:
            return None
        return message.to_dict()

    def list_messages(self, group_slug, queue_slug, status=None, limit=100, offset=0):
        queue = self._get_queue_or_raise(group_slug, queue_slug)
        query = QueueMessage.query.filter_by(queue_id=queue.id)
        if status:
            query = query.filter_by(status=status)
        messages = query.order_by(QueueMessage.created_at.desc()).limit(limit).offset(offset).all()
        return [m.to_dict() for m in messages]

    def get_stats(self, group_slug=None, queue_slug=None):
        query = db.session.query(
            QueueMessage.status,
            func.count(QueueMessage.id),
        ).group_by(QueueMessage.status)

        if queue_slug:
            queue = self._get_queue_or_raise(group_slug, queue_slug)
            query = query.filter(QueueMessage.queue_id == queue.id)
        elif group_slug:
            group = QueueGroup.query.filter_by(slug=group_slug).first()
            if not group:
                raise QueueBusError('Group not found', 404)
            query = query.filter(QueueMessage.group_id == group.id)

        counts = {status: 0 for status in (
            QueueMessage.STATUS_PENDING,
            QueueMessage.STATUS_IN_FLIGHT,
            QueueMessage.STATUS_COMPLETED,
            QueueMessage.STATUS_FAILED,
            QueueMessage.STATUS_DEAD_LETTER,
        )}
        for status, count in query.all():
            counts[status] = count

        result = {'messages': counts}
        if queue_slug:
            result['total'] = sum(counts.values())
        return result

    # ------------------------------------------------------------------
    # Helpers
    # ------------------------------------------------------------------
    def _get_queue(self, group_slug, queue_slug):
        return Queue.query.join(QueueGroup).filter(
            QueueGroup.slug == group_slug,
            Queue.slug == queue_slug,
        ).first()

    def _get_queue_or_raise(self, group_slug, queue_slug):
        queue = self._get_queue(group_slug, queue_slug)
        if not queue:
            raise QueueBusError('Queue not found', 404)
        return queue

    def _get_message(self, group_slug, queue_slug, message_id):
        return QueueMessage.query.join(Queue).join(QueueGroup).filter(
            QueueGroup.slug == group_slug,
            Queue.slug == queue_slug,
            QueueMessage.id == message_id,
        ).first()

    def _get_message_or_raise(self, group_slug, queue_slug, message_id):
        message = self._get_message(group_slug, queue_slug, message_id)
        if not message:
            raise QueueBusError('Message not found', 404)
        return message


def _valid_slug(slug):
    """Slugs are lowercase alphanumeric plus hyphen/underscore."""
    if not slug:
        return False
    if slug.startswith('-') or slug.startswith('_'):
        return False
    return all(c.isalnum() or c in '-_' for c in slug)
