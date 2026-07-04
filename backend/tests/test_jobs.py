"""Tests for the unified job system (app/jobs)."""
import pytest

from app import db
from app.jobs import registry
from app.jobs.models import Job, ScheduledJob
from app.jobs.service import JobService, ScheduledJobService, GROUP_SLUG, QUEUE_SLUG
from app.jobs.consumer import JobConsumer
from app.jobs.scheduler import JobScheduler
from app.queue_bus.service import QueueBusService
from app.queue_bus.models import QueueMessage


@pytest.fixture(autouse=True)
def reset_jobs(app):
    """Clean broker + handler registry before each test (after create_app has
    registered the built-in handlers)."""
    QueueBusService.reset_broker()
    registry.clear()
    yield
    registry.clear()


def _drain_once(consumer=None):
    """Receive and process exactly one job message; return it processed."""
    consumer = consumer or JobConsumer()
    messages = QueueBusService.receive(GROUP_SLUG, QUEUE_SLUG, max_messages=1)
    assert messages, 'expected a queued job message'
    consumer.process_message(messages[0])
    return messages[0]


class TestEnqueueAndRun:
    def test_enqueue_creates_job_and_message(self, app):
        job = JobService.enqueue('test.noop', {'x': 1})
        assert job.id is not None
        assert job.status == Job.STATUS_PENDING
        assert Job.query.count() == 1
        # A thin {job_id} pointer is on the bus.
        msgs = QueueBusService.list_messages(GROUP_SLUG, QUEUE_SLUG)
        assert len(msgs) == 1
        assert msgs[0]['payload'] == {'job_id': job.id}

    def test_handler_runs_and_succeeds(self, app):
        seen = {}

        @registry.handler('test.ok')
        def _ok(job):
            seen['payload'] = job.get_payload()
            return {'done': True}

        job = JobService.enqueue('test.ok', {'x': 42})
        _drain_once()

        refreshed = Job.query.get(job.id)
        assert refreshed.status == Job.STATUS_SUCCEEDED
        assert refreshed.get_result() == {'done': True}
        assert refreshed.completed_at is not None
        assert seen['payload'] == {'x': 42}

    def test_unregistered_kind_fails_without_retrying(self, app):
        job = JobService.enqueue('test.no_handler', {}, max_attempts=3)
        msg = _drain_once()

        refreshed = Job.query.get(job.id)
        assert refreshed.status == Job.STATUS_FAILED
        assert 'No handler' in (refreshed.error_message or '')
        # Message completed (not left to retry an unroutable job).
        qm = QueueBusService.get_message(GROUP_SLUG, QUEUE_SLUG, msg['id'])
        assert qm['status'] == QueueMessage.STATUS_COMPLETED

    def test_failure_retries_then_dead_letters_to_failed(self, app):
        @registry.handler('test.boom')
        def _boom(job):
            raise RuntimeError('kaboom')

        # max_attempts=1 → the first failure exhausts attempts → dead-letter.
        job = JobService.enqueue('test.boom', {}, max_attempts=1)
        msg = _drain_once()

        refreshed = Job.query.get(job.id)
        assert refreshed.status == Job.STATUS_FAILED
        assert 'kaboom' in (refreshed.error_message or '')
        qm = QueueBusService.get_message(GROUP_SLUG, QUEUE_SLUG, msg['id'])
        assert qm['status'] == QueueMessage.STATUS_DEAD_LETTER

    def test_failure_with_attempts_left_stays_pending(self, app):
        @registry.handler('test.flaky')
        def _flaky(job):
            raise RuntimeError('again')

        job = JobService.enqueue('test.flaky', {}, max_attempts=3)
        _drain_once()

        refreshed = Job.query.get(job.id)
        # Not terminal yet — the queue will redeliver after backoff.
        assert refreshed.status == Job.STATUS_PENDING
        assert refreshed.status not in Job.TERMINAL_STATUSES or refreshed.status == Job.STATUS_PENDING


class TestCancelAndRetry:
    def test_cancel_prevents_execution(self, app):
        ran = {'count': 0}

        @registry.handler('test.cancelme')
        def _h(job):
            ran['count'] += 1

        job = JobService.enqueue('test.cancelme', {})
        JobService.cancel(job.id)
        assert Job.query.get(job.id).status == Job.STATUS_CANCELLED

        _drain_once()  # consumer should skip the cancelled job
        assert ran['count'] == 0
        assert Job.query.get(job.id).status == Job.STATUS_CANCELLED

    def test_retry_requeues_failed_job(self, app):
        job = JobService.enqueue('test.no_handler', {}, max_attempts=1)
        _drain_once()
        assert Job.query.get(job.id).status == Job.STATUS_FAILED

        # Now register a handler and retry.
        @registry.handler('test.no_handler')
        def _now_ok(job):
            return 'recovered'

        JobService.retry(job.id)
        assert Job.query.get(job.id).status == Job.STATUS_PENDING
        _drain_once()
        assert Job.query.get(job.id).status == Job.STATUS_SUCCEEDED
        assert Job.query.get(job.id).get_result() == 'recovered'


class TestScheduler:
    def test_ensure_is_idempotent(self, app):
        a = ScheduledJobService.ensure('nightly', 'test.x', interval_seconds=60)
        b = ScheduledJobService.ensure('nightly', 'test.x', interval_seconds=120)
        assert a.id == b.id
        assert ScheduledJob.query.count() == 1
        assert ScheduledJob.query.get(a.id).interval_seconds == 120

    def test_tick_fires_due_schedule_and_advances(self, app):
        # startup_delay 0 → next_run_at = now → immediately due.
        scheduled = ScheduledJobService.ensure(
            'due-now', 'test.tick', interval_seconds=3600, startup_delay_seconds=0)
        before = scheduled.next_run_at

        fired = JobScheduler().tick()
        assert fired == 1

        # A job was enqueued for the schedule, and next_run advanced past now.
        job = Job.query.filter_by(kind='test.tick').first()
        assert job is not None
        assert job.owner_type == 'schedule'
        assert job.owner_id == 'due-now'
        refreshed = ScheduledJob.query.get(scheduled.id)
        assert refreshed.next_run_at > before
        assert refreshed.last_job_id == job.id
        # No longer due.
        assert JobScheduler().tick() == 0

    def test_disabled_schedule_does_not_fire(self, app):
        scheduled = ScheduledJobService.ensure(
            'off', 'test.tick', interval_seconds=60, startup_delay_seconds=0)
        ScheduledJobService.set_enabled(scheduled.id, False)
        assert JobScheduler().tick() == 0


class TestBuiltins:
    def test_register_and_seed(self, app):
        from app.jobs import builtin_handlers
        builtin_handlers.register_builtin_handlers()
        kinds = registry.registered_kinds()
        assert 'builtin.auto_sync' in kinds
        assert 'builtin.health_check' in kinds
        assert 'builtin.backup_scheduler' in kinds
        assert 'builtin.extension_updates' in kinds
        assert len([k for k in kinds if k.startswith('builtin.')]) == 10

        builtin_handlers.seed_builtin_schedules()
        assert ScheduledJob.query.count() == 10
        # Seeding twice doesn't duplicate.
        builtin_handlers.seed_builtin_schedules()
        assert ScheduledJob.query.count() == 10


class TestApi:
    def test_list_and_get_via_api(self, client, auth_headers, app):
        job = JobService.enqueue('test.api', {'hello': 'world'})

        resp = client.get('/api/v1/jobs', headers=auth_headers)
        assert resp.status_code == 200
        assert any(j['id'] == job.id for j in resp.get_json()['jobs'])

        resp = client.get(f'/api/v1/jobs/{job.id}', headers=auth_headers)
        assert resp.status_code == 200
        assert resp.get_json()['job']['payload'] == {'hello': 'world'}

    def test_stats_endpoint(self, client, auth_headers, app):
        JobService.enqueue('test.api', {})
        resp = client.get('/api/v1/jobs/stats', headers=auth_headers)
        assert resp.status_code == 200
        assert resp.get_json()['total'] >= 1

    def test_cancel_via_api(self, client, auth_headers, app):
        job = JobService.enqueue('test.api', {})
        resp = client.post(f'/api/v1/jobs/{job.id}/cancel', headers=auth_headers)
        assert resp.status_code == 200
        assert resp.get_json()['job']['status'] == Job.STATUS_CANCELLED

    def test_scheduled_listing(self, client, auth_headers, app):
        ScheduledJobService.ensure('api-sched', 'test.x', interval_seconds=60)
        resp = client.get('/api/v1/jobs/scheduled', headers=auth_headers)
        assert resp.status_code == 200
        assert any(s['name'] == 'api-sched' for s in resp.get_json()['scheduled'])


class TestDeploymentInstallHandler:
    """Phase 5 — template installs run as a 'deploy.install' unified job rather
    than a one-off thread. run_job is patched so these exercise the wiring, not
    real Docker/templates."""

    def test_register_jobs_adds_handler(self, app):
        from app.services.deployment_job_service import DeploymentJobService, JOB_KIND
        DeploymentJobService.register_jobs()
        assert JOB_KIND == 'deploy.install'
        assert registry.is_registered('deploy.install')

    def test_handler_success_marks_job_succeeded(self, app, monkeypatch):
        from app.services.deployment_job_service import DeploymentJobService
        calls = {}

        def fake_run_job(job_id):
            calls['job_id'] = job_id
            return {'success': True, 'app_id': 7, 'app_name': 'blog'}

        monkeypatch.setattr(DeploymentJobService, 'run_job', staticmethod(fake_run_job))
        DeploymentJobService.register_jobs()

        unified = JobService.enqueue('deploy.install', {'deployment_job_id': 'dep-1'}, max_attempts=1)
        _drain_once()

        refreshed = Job.query.get(unified.id)
        assert refreshed.status == Job.STATUS_SUCCEEDED
        assert refreshed.get_result()['app_id'] == 7
        assert calls['job_id'] == 'dep-1'

    def test_handler_failure_marks_job_failed(self, app, monkeypatch):
        from app.services.deployment_job_service import DeploymentJobService

        monkeypatch.setattr(DeploymentJobService, 'run_job',
                            staticmethod(lambda job_id: {'success': False, 'error': 'compose boom'}))
        DeploymentJobService.register_jobs()

        unified = JobService.enqueue('deploy.install', {'deployment_job_id': 'dep-2'}, max_attempts=1)
        _drain_once()

        refreshed = Job.query.get(unified.id)
        assert refreshed.status == Job.STATUS_FAILED
        assert 'compose boom' in (refreshed.error_message or '')

    def test_install_template_enqueues_job_instead_of_thread(self, app, monkeypatch):
        from app.services.deployment_job_service import DeploymentJobService
        from app.services.template_service import TemplateService
        from app.models.deployment_job import DeploymentJob

        plan = {
            'app_name': 'demo', 'app_path': '/tmp/serverkit-nonexistent-demo-xyz',
            'port': 8123, 'template_name': 'nginx', 'template_id': 'nginx',
            'steps': [{'type': 'log', 'name': 'step 1'}],
        }
        monkeypatch.setattr(
            TemplateService, 'build_install_plan',
            staticmethod(lambda **kw: {'success': True, 'plan': plan, 'app_path': plan['app_path']}))

        ran = {'called': False}

        def fake_run_job(job_id):
            ran['called'] = True
            return {'success': True}

        monkeypatch.setattr(DeploymentJobService, 'run_job', staticmethod(fake_run_job))
        DeploymentJobService.register_jobs()

        result = DeploymentJobService.install_template(
            template_id='nginx', app_name='demo', user_variables={}, user_id=1,
            server_id='local', wait=False,
        )
        assert result['success'] is True
        dep_id = result['job_id']

        # The DeploymentJob exists and is still pending — proof it was enqueued,
        # not run inline on a thread (no consumer runs under testing config).
        dep = DeploymentJob.query.get(dep_id)
        assert dep is not None and dep.status == 'pending'
        assert ran['called'] is False

        # A unified deploy.install job points back at it, with shared correlation.
        unified = Job.query.filter_by(kind='deploy.install', owner_id=dep_id).first()
        assert unified is not None
        assert unified.get_payload() == {'deployment_job_id': dep_id}
        assert unified.max_attempts == 1
        assert unified.correlation_id == dep.correlation_id


class TestWorkflowExecutionHandler:
    """Phase 6 — workflow executions run as a 'workflow.execute' unified job
    instead of inline/threaded. Empty-node workflows run for real to success;
    the failure path patches _run_execution to blow up."""

    def _make_workflow(self, nodes='[]', edges='[]'):
        from app.models.workflow import Workflow
        wf = Workflow(name='wf-test', user_id=1, nodes=nodes, edges=edges,
                      is_active=True, trigger_type='manual')
        db.session.add(wf)
        db.session.commit()
        return wf

    def test_register_jobs_adds_handler(self, app):
        from app.services.workflow_engine import WorkflowEngine, WORKFLOW_JOB_KIND
        WorkflowEngine.register_jobs()
        assert WORKFLOW_JOB_KIND == 'workflow.execute'
        assert registry.is_registered('workflow.execute')

    def test_enqueue_creates_running_row_and_job(self, app):
        from app.services.workflow_engine import WorkflowEngine
        from app.models.workflow import WorkflowExecution
        wf = self._make_workflow()

        execution_id = WorkflowEngine.enqueue_execution(wf.id, trigger_type='manual')

        ex = WorkflowExecution.query.get(execution_id)
        assert ex is not None and ex.status == 'running'
        unified = Job.query.filter_by(kind='workflow.execute', owner_id=str(wf.id)).first()
        assert unified is not None
        assert unified.get_payload() == {'execution_id': execution_id}
        assert unified.max_attempts == 1

    def test_handler_runs_empty_workflow_to_success(self, app):
        from app.services.workflow_engine import WorkflowEngine
        from app.models.workflow import WorkflowExecution
        WorkflowEngine.register_jobs()
        wf = self._make_workflow(nodes='[]', edges='[]')

        execution_id = WorkflowEngine.enqueue_execution(wf.id)
        _drain_once()

        assert WorkflowExecution.query.get(execution_id).status == 'success'
        unified = Job.query.filter_by(kind='workflow.execute').first()
        assert unified.status == Job.STATUS_SUCCEEDED

    def test_handler_failure_marks_job_failed(self, app, monkeypatch):
        from app.services.workflow_engine import WorkflowEngine
        from app.models.workflow import WorkflowExecution

        def boom(execution_id, nodes, edges):
            raise RuntimeError('node blew up')

        monkeypatch.setattr(WorkflowEngine, '_run_execution', staticmethod(boom))
        WorkflowEngine.register_jobs()
        wf = self._make_workflow(nodes='[{"id":"a","type":"trigger","data":{}}]', edges='[]')

        execution_id = WorkflowEngine.enqueue_execution(wf.id)
        _drain_once()

        assert WorkflowExecution.query.get(execution_id).status == 'failed'
        unified = Job.query.filter_by(kind='workflow.execute').first()
        assert unified.status == Job.STATUS_FAILED


class TestBackupScheduleHandler:
    """Phase 7 — scheduled backups run as 'backup.run' unified jobs, enqueued by
    the builtin.backup_scheduler tick (replacing the orphaned daemon loop).
    BackupService internals are patched so these exercise the wiring."""

    def test_register_jobs_adds_handler(self, app):
        from app.services.backup_service import BackupService, BACKUP_JOB_KIND
        BackupService.register_jobs()
        assert BACKUP_JOB_KIND == 'backup.run'
        assert registry.is_registered('backup.run')

    def test_check_schedules_enqueues_only_due_enabled(self, app, monkeypatch):
        from datetime import datetime
        from app.services.backup_service import BackupService

        now_hm = datetime.now().strftime('%H:%M')
        cfg = {'enabled': True, 'schedules': [
            {'id': 'b1', 'enabled': True, 'schedule_time': now_hm, 'days': ['daily'], 'name': 'nightly'},
            {'id': 'b2', 'enabled': False, 'schedule_time': now_hm, 'days': ['daily'], 'name': 'off'},
        ]}
        monkeypatch.setattr(BackupService, 'get_config', staticmethod(lambda: cfg))
        monkeypatch.setattr(BackupService, 'save_config', staticmethod(lambda c: None))
        monkeypatch.setattr(BackupService, 'cleanup_old_backups', staticmethod(lambda *a, **k: None))

        BackupService.check_backup_schedules()

        due = Job.query.filter_by(kind='backup.run').all()
        assert len(due) == 1
        assert due[0].owner_id == 'b1'
        assert due[0].get_payload() == {'schedule_id': 'b1'}
        assert due[0].max_attempts == 1

    def test_check_schedules_inert_when_disabled(self, app, monkeypatch):
        from datetime import datetime
        from app.services.backup_service import BackupService

        now_hm = datetime.now().strftime('%H:%M')
        cfg = {'enabled': False, 'schedules': [
            {'id': 'b1', 'enabled': True, 'schedule_time': now_hm, 'days': ['daily'], 'name': 'nightly'},
        ]}
        monkeypatch.setattr(BackupService, 'get_config', staticmethod(lambda: cfg))
        BackupService.check_backup_schedules()
        assert Job.query.filter_by(kind='backup.run').count() == 0

    def test_handler_success_and_failure(self, app, monkeypatch):
        from app.services.backup_service import BackupService

        monkeypatch.setattr(BackupService, '_run_scheduled_backup', staticmethod(lambda sched: None))
        BackupService.register_jobs()

        # Success: schedule ends up 'success'.
        monkeypatch.setattr(BackupService, 'get_config',
                            staticmethod(lambda: {'schedules': [{'id': 'b1', 'name': 'n', 'last_status': 'success'}]}))
        ok = JobService.enqueue('backup.run', {'schedule_id': 'b1'}, max_attempts=1)
        _drain_once()
        assert Job.query.get(ok.id).status == Job.STATUS_SUCCEEDED

        # Failure: schedule ends up 'failed' -> handler raises -> job failed.
        monkeypatch.setattr(BackupService, 'get_config',
                            staticmethod(lambda: {'schedules': [{'id': 'b2', 'name': 'n', 'last_status': 'failed'}]}))
        bad = JobService.enqueue('backup.run', {'schedule_id': 'b2'}, max_attempts=1)
        _drain_once()
        assert Job.query.get(bad.id).status == Job.STATUS_FAILED


class TestWorkflowEventDispatch:
    """Event-bus cleanup — WorkflowEventBus.emit enqueues a 'workflow.dispatch'
    job instead of spawning a per-event thread; the handler fans out to matching
    event-subscribed workflows."""

    def _make_event_workflow(self, event_type):
        import json as _json
        from app.models.workflow import Workflow
        wf = Workflow(name=f'evt-{event_type}', user_id=1, nodes='[]', edges='[]',
                      is_active=True, trigger_type='event',
                      trigger_config=_json.dumps({'eventType': event_type}))
        db.session.add(wf)
        db.session.commit()
        return wf

    def test_register_adds_dispatch_handler(self, app):
        from app.services.workflow_engine import WorkflowEngine
        WorkflowEngine.register_jobs()
        assert registry.is_registered('workflow.dispatch')

    def test_emit_enqueues_dispatch_job(self, app):
        from app.services.workflow_engine import WorkflowEventBus
        WorkflowEventBus.emit('high_cpu', {'value': 95})
        evt_jobs = Job.query.filter_by(kind='workflow.dispatch').all()
        assert len(evt_jobs) == 1
        assert evt_jobs[0].get_payload() == {'event_type': 'high_cpu', 'data': {'value': 95}}
        assert evt_jobs[0].owner_id == 'high_cpu'

    def test_dispatch_fans_out_to_matching_workflow_only(self, app):
        from app.services.workflow_engine import WorkflowEngine, WorkflowEventBus
        from app.models.workflow import WorkflowExecution
        WorkflowEngine.register_jobs()
        match = self._make_event_workflow('high_cpu')
        self._make_event_workflow('git_push')  # different event — must be ignored

        WorkflowEventBus.emit('high_cpu', {'value': 95})
        _drain_once()  # process workflow.dispatch -> enqueues workflow.execute

        execs = WorkflowExecution.query.all()
        assert len(execs) == 1
        assert execs[0].workflow_id == match.id
        assert execs[0].trigger_type == 'event'
        assert Job.query.filter_by(kind='workflow.execute').count() == 1
