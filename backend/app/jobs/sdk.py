"""Plugin/consumer-facing SDK for the unified job system.

    from app.plugins_sdk import jobs

    jobs.register('myplugin.reindex', handler_fn)
    jobs.enqueue('myplugin.reindex', {'site_id': 5})
    jobs.schedule('myplugin-nightly', 'myplugin.reindex', cron='0 3 * * *')
"""
from app.jobs import registry
from app.jobs.service import JobService, ScheduledJobService


class JobsSdk:
    """Stable jobs surface for plugins and internal callers."""

    def register(self, kind, handler, replace=False):
        """Register a handler ``fn(job) -> result`` for a job ``kind``."""
        return registry.register(kind, handler, replace=replace)

    def enqueue(self, kind, payload=None, max_attempts=3, priority=0, delay_ms=0,
                owner_type=None, owner_id=None):
        job = JobService.enqueue(
            kind, payload=payload, max_attempts=max_attempts, priority=priority,
            delay_ms=delay_ms, owner_type=owner_type, owner_id=owner_id,
        )
        return job.to_dict()

    def get(self, job_id):
        job = JobService.get(job_id)
        return job.to_dict(include_payload=True) if job else None

    def cancel(self, job_id):
        job = JobService.cancel(job_id)
        return job.to_dict() if job else None

    def schedule(self, name, kind, interval_seconds=None, cron=None, payload=None,
                 max_attempts=1, startup_delay_seconds=0):
        scheduled = ScheduledJobService.ensure(
            name, kind, interval_seconds=interval_seconds, cron=cron, payload=payload,
            max_attempts=max_attempts, startup_delay_seconds=startup_delay_seconds,
        )
        return scheduled.to_dict()
