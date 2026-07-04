"""Producer + management API for the unified job system.

``JobService`` is the one entry point to enqueue work; ``ScheduledJobService``
manages periodic schedules. Both are non-blocking: they persist a row and (for
jobs) publish a thin pointer onto the Queue Bus.
"""
import logging
from datetime import datetime, timedelta

from app import db
from app.jobs.models import Job, ScheduledJob
from app.queue_bus.service import QueueBusService

logger = logging.getLogger(__name__)

GROUP_SLUG = 'serverkit-system'
QUEUE_SLUG = 'jobs'
# Jobs can run longer than a webhook POST, so give them a wider visibility
# window before the queue considers a message abandoned.
QUEUE_CONFIG = {'max_attempts': 3, 'visibility_timeout_ms': 120000}


def _new_correlation_id():
    try:
        from app.services.telemetry_service import generate_correlation_id
        return generate_correlation_id()
    except Exception:
        import uuid
        return uuid.uuid4().hex


class JobService:

    @classmethod
    def enqueue(cls, kind, payload=None, max_attempts=3, priority=0, delay_ms=0,
                owner_type=None, owner_id=None, correlation_id=None):
        """Persist a Job and publish it onto the Queue Bus. Returns the Job."""
        job = Job(
            kind=kind,
            status=Job.STATUS_PENDING,
            max_attempts=max_attempts,
            priority=priority,
            owner_type=owner_type,
            owner_id=str(owner_id) if owner_id is not None else None,
            correlation_id=correlation_id or _new_correlation_id(),
        )
        job.set_payload(payload or {})
        if delay_ms:
            job.scheduled_at = datetime.utcnow() + timedelta(milliseconds=delay_ms)
        db.session.add(job)
        db.session.commit()
        cls._publish(job, delay_ms=delay_ms)
        return job

    @classmethod
    def _publish(cls, job, delay_ms=0):
        QueueBusService.ensure_queue(GROUP_SLUG, QUEUE_SLUG, config=QUEUE_CONFIG)
        msg = QueueBusService.send(
            GROUP_SLUG, QUEUE_SLUG, {'job_id': job.id},
            priority=job.priority or 0, delay_ms=delay_ms,
            max_attempts=job.max_attempts,
        )
        if isinstance(msg, dict):
            job.queue_message_id = msg.get('id')
            db.session.commit()
        return msg

    @classmethod
    def get(cls, job_id):
        return Job.query.get(job_id)

    @classmethod
    def list(cls, status=None, kind=None, owner_type=None, owner_id=None, limit=50, offset=0):
        query = Job.query
        if status:
            query = query.filter_by(status=status)
        if kind:
            query = query.filter_by(kind=kind)
        if owner_type:
            query = query.filter_by(owner_type=owner_type)
        if owner_id is not None:
            query = query.filter_by(owner_id=str(owner_id))
        return query.order_by(Job.created_at.desc()).limit(limit).offset(offset).all()

    @classmethod
    def cancel(cls, job_id):
        """Mark a pending/running job cancelled. Cannot interrupt a handler that
        is already executing — it stops future attempts and is skipped at the
        next queue pickup."""
        job = Job.query.get(job_id)
        if not job:
            return None
        if job.status in (Job.STATUS_PENDING, Job.STATUS_RUNNING):
            job.status = Job.STATUS_CANCELLED
            job.completed_at = datetime.utcnow()
            db.session.commit()
        return job

    @classmethod
    def retry(cls, job_id):
        """Re-enqueue a failed/cancelled job as a fresh attempt."""
        job = Job.query.get(job_id)
        if not job:
            return None
        if job.status not in (Job.STATUS_FAILED, Job.STATUS_CANCELLED):
            return job
        job.status = Job.STATUS_PENDING
        job.error_message = None
        job.started_at = None
        job.completed_at = None
        db.session.commit()
        cls._publish(job)
        return job

    @classmethod
    def stats(cls):
        from sqlalchemy import func
        by_status = dict(
            db.session.query(Job.status, func.count(Job.id)).group_by(Job.status).all()
        )
        by_kind = dict(
            db.session.query(Job.kind, func.count(Job.id)).group_by(Job.kind).all()
        )
        return {'by_status': by_status, 'by_kind': by_kind, 'total': sum(by_status.values())}

    @classmethod
    def cleanup_old(cls, max_age_seconds=86400):
        """Delete terminal jobs whose completion is older than the window."""
        cutoff = datetime.utcnow() - timedelta(seconds=max_age_seconds)
        deleted = (Job.query
                   .filter(Job.status.in_(Job.TERMINAL_STATUSES))
                   .filter(Job.completed_at.isnot(None))
                   .filter(Job.completed_at < cutoff)
                   .delete(synchronize_session=False))
        db.session.commit()
        return deleted


class ScheduledJobService:

    @classmethod
    def ensure(cls, name, kind, interval_seconds=None, cron=None, payload=None,
               max_attempts=1, startup_delay_seconds=0, enabled=True):
        """Idempotently create/update a periodic schedule keyed by ``name``.

        Updates cadence/kind on an existing row but PRESERVES its
        ``next_run_at`` / ``last_run_at`` and the admin's ``enabled`` toggle so a
        restart doesn't reset the clock. On first creation, seeds ``next_run_at``
        honoring ``startup_delay_seconds``.
        """
        scheduled = ScheduledJob.query.filter_by(name=name).first()
        created = scheduled is None
        if created:
            scheduled = ScheduledJob(name=name)
            db.session.add(scheduled)
        scheduled.kind = kind
        scheduled.schedule_kind = ScheduledJob.SCHEDULE_CRON if cron else ScheduledJob.SCHEDULE_INTERVAL
        scheduled.interval_seconds = interval_seconds
        scheduled.cron = cron
        scheduled.max_attempts = max_attempts
        if payload is not None:
            scheduled.set_payload(payload)
        if created:
            scheduled.enabled = enabled
            scheduled.next_run_at = datetime.utcnow() + timedelta(seconds=startup_delay_seconds or 0)
        db.session.commit()
        return scheduled

    @classmethod
    def due(cls, now=None):
        now = now or datetime.utcnow()
        return (ScheduledJob.query
                .filter(ScheduledJob.enabled.is_(True))
                .filter(ScheduledJob.next_run_at.isnot(None))
                .filter(ScheduledJob.next_run_at <= now)
                .all())

    @classmethod
    def fire(cls, scheduled, now=None):
        """Enqueue a Job for one schedule and advance its ``next_run_at``."""
        now = now or datetime.utcnow()
        job = JobService.enqueue(
            scheduled.kind,
            payload=scheduled.get_payload(),
            max_attempts=scheduled.max_attempts,
            owner_type='schedule',
            owner_id=scheduled.name,
        )
        job.scheduled_job_id = scheduled.id
        scheduled.last_run_at = now
        scheduled.last_job_id = job.id
        scheduled.next_run_at = scheduled.compute_next_run(now)
        db.session.commit()
        return job

    @classmethod
    def list(cls):
        return ScheduledJob.query.order_by(ScheduledJob.name.asc()).all()

    @classmethod
    def run_now(cls, scheduled_job_id):
        scheduled = ScheduledJob.query.get(scheduled_job_id)
        if not scheduled:
            return None
        return cls.fire(scheduled)

    @classmethod
    def set_enabled(cls, scheduled_job_id, enabled):
        scheduled = ScheduledJob.query.get(scheduled_job_id)
        if not scheduled:
            return None
        scheduled.enabled = bool(enabled)
        if enabled and not scheduled.next_run_at:
            scheduled.next_run_at = datetime.utcnow()
        db.session.commit()
        return scheduled
