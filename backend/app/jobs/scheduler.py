"""The single periodic ticker that replaces the per-domain scheduler threads.

It wakes on a short interval, finds due ``ScheduledJob`` rows, and enqueues a Job
for each (advancing ``next_run_at``). All cadence lives in the DB, so adding or
disabling a periodic task is a row change, not a new daemon thread.
"""
import logging
import threading
import time

from app.jobs.service import ScheduledJobService

logger = logging.getLogger(__name__)

_job_scheduler = None


class JobScheduler:
    def __init__(self, app=None, tick_seconds=15):
        self.app = app
        self.running = False
        self.tick_seconds = tick_seconds

    def start(self):
        if self.running:
            return
        self.running = True
        thread = threading.Thread(target=self._run, daemon=True, name='job-scheduler')
        thread.start()
        logger.info('Job scheduler started')

    def stop(self):
        self.running = False

    def _run(self):
        while self.running:
            try:
                with self.app.app_context():
                    self.tick()
            except Exception as e:
                logger.error(f'Job scheduler error: {e}')
            time.sleep(self.tick_seconds)

    def tick(self):
        """Enqueue all due schedules. Returns the number fired. Test-callable."""
        fired = 0
        for scheduled in ScheduledJobService.due():
            try:
                ScheduledJobService.fire(scheduled)
                fired += 1
            except Exception as e:
                logger.error(f'Failed to fire scheduled job {scheduled.name}: {e}')
                # Advance anyway so a poison schedule can't hot-loop the ticker.
                try:
                    from app import db
                    scheduled.next_run_at = scheduled.compute_next_run()
                    db.session.commit()
                except Exception:
                    db.session.rollback()
        return fired


def start_job_scheduler(app):
    """Start the singleton scheduler thread (skipped under testing config)."""
    global _job_scheduler
    if _job_scheduler is not None:
        return
    if app.config.get('ENV') == 'testing' or app.config.get('TESTING'):
        return
    scheduler = JobScheduler(app)
    scheduler.start()
    _job_scheduler = scheduler


def stop_job_scheduler():
    global _job_scheduler
    if _job_scheduler is not None:
        _job_scheduler.stop()
        _job_scheduler = None
