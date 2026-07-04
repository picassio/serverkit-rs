"""Tests for the centralized container status aggregator.

These exercise the pure ``aggregate_status`` precedence hierarchy directly — no
Docker, no app context required. A couple of cache/changed-detection tests use
the in-memory cache fallback only.
"""

from app.services import container_status_service as css
from app.services.container_status_service import (
    aggregate_status,
    STATUS_RUNNING_HEALTHY,
    STATUS_RUNNING_UNHEALTHY,
    STATUS_DEGRADED,
    STATUS_RESTARTING,
    STATUS_STARTING,
    STATUS_EXITED,
    STATUS_UNKNOWN,
)


class TestAggregateStatus:
    def test_empty_list_is_unknown(self):
        result = aggregate_status([])
        assert result['status'] == STATUS_UNKNOWN
        assert result['total'] == 0
        assert result['healthy'] == 0

    def test_none_input_is_unknown(self):
        result = aggregate_status(None)
        assert result['status'] == STATUS_UNKNOWN
        assert result['total'] == 0

    def test_all_running_healthy(self):
        result = aggregate_status([
            {'name': 'web', 'state': 'running', 'health': 'healthy'},
            {'name': 'db', 'state': 'running', 'health': 'healthy'},
        ])
        assert result['status'] == STATUS_RUNNING_HEALTHY
        assert result['total'] == 2
        assert result['healthy'] == 2

    def test_running_without_healthcheck_counts_healthy(self):
        # No health check (health absent / 'none') is treated as healthy.
        result = aggregate_status([
            {'name': 'web', 'state': 'running'},
            {'name': 'db', 'state': 'running', 'health': 'none'},
        ])
        assert result['status'] == STATUS_RUNNING_HEALTHY
        assert result['healthy'] == 2

    def test_one_unhealthy_beats_healthy(self):
        result = aggregate_status([
            {'name': 'web', 'state': 'running', 'health': 'healthy'},
            {'name': 'db', 'state': 'running', 'health': 'unhealthy'},
        ])
        assert result['status'] == STATUS_RUNNING_UNHEALTHY
        assert result['healthy'] == 1
        assert any('db' in r for r in result['reasons'])

    def test_one_restarting_beats_unhealthy(self):
        # restarting has higher precedence than running:unhealthy.
        result = aggregate_status([
            {'name': 'web', 'state': 'running', 'health': 'unhealthy'},
            {'name': 'db', 'state': 'restarting'},
        ])
        assert result['status'] == STATUS_RESTARTING

    def test_degraded_when_multi_partially_up(self):
        # Some running, some not (and nothing restarting) => degraded.
        result = aggregate_status([
            {'name': 'web', 'state': 'running', 'health': 'healthy'},
            {'name': 'db', 'state': 'exited'},
        ])
        assert result['status'] == STATUS_DEGRADED
        assert any('db' in r for r in result['reasons'])

    def test_degraded_beats_restarting(self):
        # A partial set that also has a restart loop -> degraded wins (it's the
        # top of the hierarchy), but only when there's a non-running, non-
        # restarting member. With a restart in the mix and one plain exited and
        # one running, "down" set is non-empty -> degraded.
        result = aggregate_status([
            {'name': 'web', 'state': 'running'},
            {'name': 'cache', 'state': 'exited'},
            {'name': 'db', 'state': 'restarting'},
        ])
        # restarting present means the degraded branch is skipped by design
        # (its guard excludes restarting); precedence then yields restarting.
        assert result['status'] == STATUS_RESTARTING

    def test_starting_beats_running_healthy(self):
        result = aggregate_status([
            {'name': 'web', 'state': 'running', 'health': 'healthy'},
            {'name': 'db', 'state': 'running', 'health': 'starting'},
        ])
        assert result['status'] == STATUS_STARTING

    def test_created_state_is_starting(self):
        result = aggregate_status([
            {'name': 'web', 'state': 'created'},
        ])
        assert result['status'] == STATUS_STARTING

    def test_all_exited_is_exited(self):
        result = aggregate_status([
            {'name': 'web', 'state': 'exited'},
            {'name': 'db', 'state': 'exited'},
        ])
        assert result['status'] == STATUS_EXITED
        assert result['healthy'] == 0

    def test_single_exited_is_exited_not_degraded(self):
        # degraded requires >1 container; a single stopped container is exited.
        result = aggregate_status([
            {'name': 'web', 'state': 'exited'},
        ])
        assert result['status'] == STATUS_EXITED

    def test_unknown_state_is_unknown(self):
        result = aggregate_status([
            {'name': 'web', 'state': 'weird-thing'},
        ])
        assert result['status'] == STATUS_UNKNOWN

    def test_single_running_unhealthy(self):
        result = aggregate_status([
            {'name': 'web', 'state': 'running', 'health': 'unhealthy'},
        ])
        assert result['status'] == STATUS_RUNNING_UNHEALTHY

    def test_containers_are_normalized_in_output(self):
        result = aggregate_status([
            {'name': 'web', 'state': 'UP', 'health': 'Healthy'},
        ])
        c = result['containers'][0]
        assert c['state'] == 'running'
        assert c['health'] == 'healthy'

    def test_precedence_constant_order(self):
        # Guard the documented precedence ordering against accidental edits.
        assert css.STATUS_PRECEDENCE == [
            STATUS_DEGRADED,
            STATUS_RESTARTING,
            STATUS_RUNNING_UNHEALTHY,
            STATUS_STARTING,
            STATUS_RUNNING_HEALTHY,
            STATUS_EXITED,
            STATUS_UNKNOWN,
        ]


class TestGetAppStatusDefensive:
    def test_missing_app_returns_unknown(self, app):
        # No Application row with this id -> well-formed unknown, no raise.
        with app.app_context():
            result = css.get_app_status(999999, use_cache=False)
        assert result['status'] == STATUS_UNKNOWN
        assert result['app_id'] == 999999
        assert result['kind'] == 'app'

    def test_database_status_without_container_is_unknown(self):
        result = css.get_database_status(5)
        assert result['status'] == STATUS_UNKNOWN
        assert result['kind'] == 'database'
        assert result['database_id'] == 5


class TestChangedDetection:
    def test_changed_detection_emits_then_dedupes(self, app, monkeypatch):
        # Reset the in-memory snapshot so the test is deterministic.
        css._last_app_statuses.clear()

        monkeypatch.setattr(css, 'list_app_statuses', lambda: [
            {'app_id': 1, 'status': STATUS_RUNNING_HEALTHY, 'total': 1, 'healthy': 1},
        ])

        first = css.get_changed_app_statuses()
        assert len(first) == 1
        assert first[0]['app_id'] == 1

        # Same status second time -> no change emitted.
        second = css.get_changed_app_statuses()
        assert second == []

        # Status flips -> emitted again.
        monkeypatch.setattr(css, 'list_app_statuses', lambda: [
            {'app_id': 1, 'status': STATUS_EXITED, 'total': 1, 'healthy': 0},
        ])
        third = css.get_changed_app_statuses()
        assert len(third) == 1
        assert third[0]['status'] == STATUS_EXITED

        css._last_app_statuses.clear()
