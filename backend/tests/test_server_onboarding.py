"""Tests for the server onboarding state machine (Phase 1).

Under the testing config the job consumer is disabled, so the service is driven
synchronously: `start()` validates + advances all the way to `ready` (the steps
after `validating` are idempotent stubs that chain via `advance`).
"""
import pytest

from app import db
from app.models.audit_log import AuditLog
from app.models.server import Server
from app.models.server_onboarding_log import ServerOnboardingLog
from app.services.server_onboarding_service import ServerOnboardingService as SOS


def _make_server(**overrides):
    data = dict(
        name='Test Server',
        hostname='test.example.com',
        ip_address='203.0.113.10',
    )
    data.update(overrides)
    server = Server(**data)
    db.session.add(server)
    db.session.commit()
    return server


def test_start_runs_full_pipeline_to_ready(app):
    with app.app_context():
        server = _make_server()
        assert server.onboarding_state in (None, 'pending')

        status = SOS.start(server.id)

        # Synchronous in testing — should reach the terminal ready state.
        assert status['state'] == SOS.STATE_READY
        assert status['is_terminal'] is True

        refreshed = Server.query.get(server.id)
        assert refreshed.onboarding_state == SOS.STATE_READY
        assert refreshed.onboarding_updated_at is not None

        # Logs were written for each lifecycle step.
        states_logged = {
            row.state for row in
            ServerOnboardingLog.query.filter_by(server_id=server.id).all()
        }
        assert SOS.STATE_VALIDATING in states_logged
        assert SOS.STATE_INSTALLING_PREREQS in states_logged
        assert SOS.STATE_INSTALLING_DOCKER in states_logged
        assert SOS.STATE_PAIRING_AGENT in states_logged
        assert SOS.STATE_READY in states_logged


def test_progress_snapshot_mirrored_on_server(app):
    with app.app_context():
        server = _make_server()
        SOS.start(server.id)

        refreshed = Server.query.get(server.id)
        progress = refreshed._onboarding_progress_list()
        assert isinstance(progress, list)
        assert len(progress) > 0
        # Each snapshot entry carries the to_dict() shape.
        assert {'state', 'status', 'message'} <= set(progress[0].keys())

        # to_dict surfaces the new fields.
        as_dict = refreshed.to_dict()
        assert as_dict['onboarding_state'] == SOS.STATE_READY
        assert isinstance(as_dict['onboarding_progress'], list)
        assert as_dict['onboarding_updated_at'] is not None


def test_validation_fails_without_endpoint_or_agent(app):
    with app.app_context():
        # No hostname/ip and no agent — validation must fail.
        server = _make_server(hostname=None, ip_address=None, agent_id=None)

        status = SOS.start(server.id)

        assert status['state'] == SOS.STATE_FAILED
        refreshed = Server.query.get(server.id)
        assert refreshed.onboarding_state == SOS.STATE_FAILED

        failed_logs = ServerOnboardingLog.query.filter_by(
            server_id=server.id, status=ServerOnboardingLog.STATUS_FAILED
        ).all()
        assert len(failed_logs) >= 1
        assert failed_logs[0].state == SOS.STATE_VALIDATING


def test_retry_recovers_from_failed(app):
    with app.app_context():
        server = _make_server(hostname=None, ip_address=None, agent_id=None)
        SOS.start(server.id)
        assert Server.query.get(server.id).onboarding_state == SOS.STATE_FAILED

        # Give it a reachable endpoint, then retry — should now complete.
        server = Server.query.get(server.id)
        server.hostname = 'recovered.example.com'
        db.session.commit()

        status = SOS.retry(server.id)
        assert status['state'] == SOS.STATE_READY
        assert Server.query.get(server.id).onboarding_state == SOS.STATE_READY

        # A "Retrying onboarding" log row exists.
        retry_logs = [
            r for r in ServerOnboardingLog.query.filter_by(server_id=server.id).all()
            if r.message and 'Retry' in r.message
        ]
        assert len(retry_logs) >= 1


def test_retry_on_non_failed_is_noop(app):
    with app.app_context():
        server = _make_server()
        SOS.start(server.id)  # -> ready
        status = SOS.retry(server.id)
        # Already ready; retry just reports current status.
        assert status['state'] == SOS.STATE_READY


def test_transition_validation_helper(app):
    with app.app_context():
        assert SOS.is_valid_transition(SOS.STATE_PENDING, SOS.STATE_VALIDATING)
        assert SOS.is_valid_transition(
            SOS.STATE_VALIDATING, SOS.STATE_INSTALLING_PREREQS)
        assert not SOS.is_valid_transition(
            SOS.STATE_PENDING, SOS.STATE_READY)
        # Any non-ready state can fail.
        assert SOS.is_valid_transition(
            SOS.STATE_INSTALLING_DOCKER, SOS.STATE_FAILED)


def test_get_status_unknown_server_raises(app):
    with app.app_context():
        with pytest.raises(ValueError):
            SOS.get_status('does-not-exist')


def test_register_jobs_is_callable(app):
    with app.app_context():
        # Should not raise; registers the advance handler.
        SOS.register_jobs()
        from app.jobs import registry
        from app.services.server_onboarding_service import ONBOARDING_JOB_KIND
        assert registry.is_registered(ONBOARDING_JOB_KIND)


# --------------------------------------------------------------------------- #
# pre_flight_check — PURE function matrices (no app context needed)
# --------------------------------------------------------------------------- #

def test_preflight_fully_compatible_host():
    result = SOS.pre_flight_check({
        'architecture': 'x86_64',
        'os_type': 'linux',
        'distro': 'ubuntu',
        'kernel': '5.15.0-89-generic',
        'cpu_cores': 4,
        'total_memory': 8 * 1024 * 1024 * 1024,
    })
    assert result['compatible'] is True
    assert result['blocking'] == []
    by_name = {c['name']: c for c in result['checks']}
    assert by_name['architecture']['ok'] is True
    assert by_name['distro']['ok'] is True
    assert by_name['kernel']['ok'] is True
    assert by_name['cpu']['ok'] is True
    assert by_name['memory']['ok'] is True


def test_preflight_arm64_alias_supported():
    result = SOS.pre_flight_check({'architecture': 'aarch64', 'os_type': 'linux'})
    assert result['compatible'] is True
    by_name = {c['name']: c for c in result['checks']}
    assert by_name['architecture']['ok'] is True


def test_preflight_unsupported_arch_blocks():
    result = SOS.pre_flight_check({'architecture': 'ppc64le', 'os_type': 'linux'})
    assert result['compatible'] is False
    assert 'architecture' in result['blocking']


def test_preflight_non_linux_os_blocks():
    result = SOS.pre_flight_check({
        'architecture': 'x86_64', 'os_type': 'windows',
    })
    assert result['compatible'] is False
    assert 'os' in result['blocking']


def test_preflight_old_kernel_blocks():
    result = SOS.pre_flight_check({
        'architecture': 'x86_64', 'os_type': 'linux', 'kernel': '2.6.32',
    })
    assert result['compatible'] is False
    assert 'kernel' in result['blocking']


def test_preflight_too_little_memory_blocks():
    result = SOS.pre_flight_check({
        'architecture': 'x86_64', 'os_type': 'linux',
        'total_memory': 128 * 1024 * 1024,  # 128 MiB
    })
    assert result['compatible'] is False
    assert 'memory' in result['blocking']


def test_preflight_zero_cpu_blocks():
    result = SOS.pre_flight_check({
        'architecture': 'x86_64', 'os_type': 'linux', 'cpu_cores': 0,
    })
    assert result['compatible'] is False
    assert 'cpu' in result['blocking']


def test_preflight_sparse_info_is_unknown_not_blocking():
    # Empty system_info: nothing is known, so nothing blocks.
    result = SOS.pre_flight_check({})
    assert result['compatible'] is True
    assert result['blocking'] == []
    # All checks are "unknown" (ok is None).
    assert all(c['ok'] is None for c in result['checks'])


def test_preflight_unrecognized_distro_blocks():
    result = SOS.pre_flight_check({
        'architecture': 'x86_64', 'os_type': 'linux', 'distro': 'plan9',
    })
    assert result['compatible'] is False
    assert 'distro' in result['blocking']


def test_preflight_none_input_safe():
    # None must not crash and must be treated as fully-unknown.
    result = SOS.pre_flight_check(None)
    assert result['compatible'] is True


def test_preflight_stringy_values_coerced():
    result = SOS.pre_flight_check({
        'architecture': 'amd64', 'os_type': 'linux',
        'cpu_cores': '2', 'total_memory': str(2 * 1024 * 1024 * 1024),
    })
    assert result['compatible'] is True
    by_name = {c['name']: c for c in result['checks']}
    assert by_name['cpu']['ok'] is True
    assert by_name['memory']['ok'] is True


# --------------------------------------------------------------------------- #
# Real-but-guarded steps + audit emission
# --------------------------------------------------------------------------- #

def test_incompatible_host_fails_at_validate(app):
    with app.app_context():
        # Reachable endpoint but a known-bad architecture -> preflight blocks.
        server = _make_server(architecture='ppc64le', os_type='linux')
        status = SOS.start(server.id)
        assert status['state'] == SOS.STATE_FAILED

        failed = ServerOnboardingLog.query.filter_by(
            server_id=server.id, status=ServerOnboardingLog.STATUS_FAILED
        ).all()
        assert any(f.state == SOS.STATE_VALIDATING for f in failed)
        # The failure detail carries the pre-flight result.
        detail = failed[0].get_detail()
        assert detail.get('preflight', {}).get('compatible') is False


def test_guarded_steps_reach_ready_on_dev(app):
    # On Windows/dev (and on Linux without the tooling) every install/pair step
    # degrades to a safe success, so the pipeline still completes.
    with app.app_context():
        server = _make_server()
        status = SOS.start(server.id)
        assert status['state'] == SOS.STATE_READY

        # Each step logged a succeeded row with a human-readable message.
        succeeded = ServerOnboardingLog.query.filter_by(
            server_id=server.id, status=ServerOnboardingLog.STATUS_SUCCEEDED
        ).all()
        states_done = {r.state for r in succeeded}
        assert SOS.STATE_INSTALLING_PREREQS in states_done
        assert SOS.STATE_INSTALLING_DOCKER in states_done
        assert SOS.STATE_PAIRING_AGENT in states_done


def test_install_prerequisites_idempotent_detail(app):
    # Calling the underlying helper twice must never raise and must return a
    # (message, detail) pair both times (degrades on dev).
    with app.app_context():
        msg1, detail1 = SOS._do_install_prerequisites()
        msg2, detail2 = SOS._do_install_prerequisites()
        assert isinstance(msg1, str) and isinstance(detail1, dict)
        assert isinstance(msg2, str) and isinstance(detail2, dict)


def test_pair_agent_records_waiting_when_not_enrolled(app):
    with app.app_context():
        # Endpoint present (so validate passes) but no agent credentials.
        server = _make_server(agent_id=None)
        SOS.start(server.id)

        pair_logs = ServerOnboardingLog.query.filter_by(
            server_id=server.id, state=SOS.STATE_PAIRING_AGENT,
            status=ServerOnboardingLog.STATUS_SUCCEEDED,
        ).all()
        assert len(pair_logs) >= 1
        detail = pair_logs[-1].get_detail()
        assert detail.get('registered') is False
        assert 'waiting' in detail


def test_audit_rows_written_for_transitions(app):
    with app.app_context():
        server = _make_server()
        SOS.start(server.id)

        actions = {a.action for a in AuditLog.query.all()}
        assert 'server.onboarding.started' in actions
        assert 'server.onboarding.step' in actions
        assert 'server.onboarding.completed' in actions

        # The started audit stashes the server id inside details (target_id is
        # an Integer column, server ids are UUID strings).
        started = AuditLog.query.filter_by(
            action='server.onboarding.started').first()
        assert started is not None
        assert started.get_details().get('server_id') == server.id


def test_audit_failure_row_written(app):
    with app.app_context():
        server = _make_server(hostname=None, ip_address=None, agent_id=None)
        SOS.start(server.id)
        actions = {a.action for a in AuditLog.query.all()}
        assert 'server.onboarding.failed' in actions
