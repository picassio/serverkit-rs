"""ServerKit unified job system.

A thin Job / WorkUnit + scheduler layer on top of the Queue Bus. A producer
calls ``JobService.enqueue(kind, payload)``; that persists a ``Job`` row and
publishes a thin ``{'job_id': ...}`` message onto the ``serverkit-system/jobs``
queue. One ``JobConsumer`` runs every job by dispatching its ``kind`` to a
registered handler, and one ``JobScheduler`` ticks all periodic work. This is
the same pattern the Notification Bus uses, generalized.

See ``docs/ORCHESTRATION_ROADMAP.md`` (local, git-ignored) for the plan.
"""
# Import the models at package-import time so they register on ``db.metadata``
# and ``MigrationService._fix_missing_columns`` -> ``db.create_all()`` picks them
# up on boot — mirrors how ``app/queue_bus`` registers its models. No Alembic
# migration is required for new tables.
from app.jobs.models import Job, ScheduledJob  # noqa: F401
from app.jobs import registry  # noqa: F401

__all__ = ['Job', 'ScheduledJob', 'registry', 'start_job_system']


def start_job_system(app, seed=None):
    """Ensure the jobs queue exists, seed built-in schedules, and start the
    consumer + scheduler daemon threads.

    No-op under testing config (mirrors the queue-bus / notification consumers),
    so the suite drives ``JobConsumer.process_message`` / ``JobScheduler.tick``
    directly instead of racing background threads against per-test databases.
    """
    if app.config.get('ENV') == 'testing' or app.config.get('TESTING'):
        return

    import logging
    from app.queue_bus.service import QueueBusService
    from app.jobs.service import GROUP_SLUG, QUEUE_SLUG, QUEUE_CONFIG
    from app.jobs.consumer import start_job_consumer
    from app.jobs.scheduler import start_job_scheduler

    with app.app_context():
        QueueBusService.ensure_queue(GROUP_SLUG, QUEUE_SLUG, config=QUEUE_CONFIG)
        if seed is not None:
            try:
                seed()
            except Exception:
                logging.getLogger(__name__).exception('Built-in schedule seeding failed')

    start_job_consumer(app)
    start_job_scheduler(app)
