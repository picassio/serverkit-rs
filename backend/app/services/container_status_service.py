"""Centralized container status aggregator.

Collapses the per-container Docker states of an app/service/database into ONE
deterministic status using a fixed priority hierarchy. The aggregation
(``aggregate_status``) is a pure function with no Docker dependency so it can be
unit-tested in isolation; the ``get_*_status`` helpers wire it to real Docker
data and a short-TTL cache.

Status vocabulary (the aggregated enum):

    running:healthy    every container running, health checks (if any) passing
    running:unhealthy  running but at least one container reports unhealthy
    degraded           a multi-container set is only partially up
                       (some running, some not) — the set isn't whole
    restarting         at least one container is in a restart loop
    starting           at least one container is starting / health=starting
    exited             nothing is running (all stopped/exited) but containers exist
    unknown            no containers, entity missing, or Docker unavailable

Priority hierarchy (highest precedence first). The aggregate is whichever of
these conditions is true first:

    degraded > restarting > running:unhealthy > starting > running:healthy
    > exited > unknown

Rationale: a partially-up set (``degraded``) is the loudest problem because the
app is wedged between states; an active restart loop is next; an explicit
unhealthy health-check beats a quiet "starting"; a clean all-running set is the
happy path; ``exited`` is a calm, fully-stopped state; ``unknown`` is the floor.
"""

import logging

logger = logging.getLogger(__name__)


# ---- Aggregated status constants -------------------------------------------
STATUS_RUNNING_HEALTHY = 'running:healthy'
STATUS_RUNNING_UNHEALTHY = 'running:unhealthy'
STATUS_DEGRADED = 'degraded'
STATUS_RESTARTING = 'restarting'
STATUS_STARTING = 'starting'
STATUS_EXITED = 'exited'
STATUS_UNKNOWN = 'unknown'

# Precedence: index 0 wins. Used both to document and to pick the aggregate.
STATUS_PRECEDENCE = [
    STATUS_DEGRADED,
    STATUS_RESTARTING,
    STATUS_RUNNING_UNHEALTHY,
    STATUS_STARTING,
    STATUS_RUNNING_HEALTHY,
    STATUS_EXITED,
    STATUS_UNKNOWN,
]

# Cache namespace + TTL. Short on purpose so the pill is "live enough" without
# hammering the Docker CLI on every render / list load.
_CACHE_PREFIX = 'container_status'
_CACHE_TTL = 8  # seconds


def _normalize_state(raw):
    """Map a raw docker state/status string to one of: running, restarting,
    starting, exited, unknown. Defensive against None / odd casing."""
    if not raw:
        return 'unknown'
    s = str(raw).strip().lower()
    if s in ('running', 'up'):
        return 'running'
    if s in ('restarting',):
        return 'restarting'
    if s in ('created', 'starting'):
        return 'starting'
    if s in ('exited', 'dead', 'stopped', 'paused', 'removing'):
        return 'exited'
    return 'unknown'


def _normalize_health(raw):
    """Map a raw docker health string to: healthy, unhealthy, starting, none."""
    if not raw:
        return 'none'
    s = str(raw).strip().lower()
    if s == 'healthy':
        return 'healthy'
    if s == 'unhealthy':
        return 'unhealthy'
    if s == 'starting':
        return 'starting'
    if s in ('none', 'no-healthcheck', 'no healthcheck'):
        return 'none'
    return 'none'


def aggregate_status(container_states):
    """Collapse a list of per-container states into one aggregated status.

    Pure function — no Docker calls. This is the single source of truth for the
    precedence hierarchy and is what the tests exercise directly.

    Args:
        container_states: list of dicts, each with at least ``state`` and an
            optional ``health`` (and optional ``name``/``service`` used only for
            human-readable reasons). Example:
                {'name': 'app-db-1', 'state': 'running', 'health': 'unhealthy'}

    Returns:
        dict: {
            'status':   <aggregated enum>,
            'total':    <container count>,
            'healthy':  <count of running + (healthy|none) containers>,
            'reasons':  [<short human strings>],
            'containers': [<normalized per-container dicts>],
        }
    """
    containers = []
    for c in (container_states or []):
        c = c or {}
        state = _normalize_state(c.get('state'))
        health = _normalize_health(c.get('health'))
        containers.append({
            'name': c.get('name') or c.get('service') or c.get('id') or '?',
            'service': c.get('service'),
            'state': state,
            'health': health,
        })

    total = len(containers)
    if total == 0:
        return {
            'status': STATUS_UNKNOWN,
            'total': 0,
            'healthy': 0,
            'reasons': ['no containers'],
            'containers': [],
        }

    running = [c for c in containers if c['state'] == 'running']
    restarting = [c for c in containers if c['state'] == 'restarting']
    starting = [c for c in containers if c['state'] == 'starting']
    # A running container counts as "healthy" unless it explicitly reports
    # unhealthy. No health-check (health='none') is treated as healthy.
    healthy = [c for c in running if c['health'] in ('healthy', 'none')]
    unhealthy = [c for c in running if c['health'] == 'unhealthy']
    health_starting = [c for c in running if c['health'] == 'starting']

    reasons = []

    # --- Apply the precedence hierarchy, highest first ---

    # degraded: a partially-up multi-container set (some running, some not).
    not_running = [c for c in containers if c['state'] != 'running']
    if total > 1 and running and not_running and not restarting:
        down = ', '.join(c['name'] for c in not_running)
        reasons.append(f'partially up — down: {down}')
        status = STATUS_DEGRADED

    # restarting: at least one container is looping.
    elif restarting:
        reasons.append('restarting: ' + ', '.join(c['name'] for c in restarting))
        status = STATUS_RESTARTING

    # running:unhealthy: running but a health check is failing.
    elif unhealthy:
        reasons.append('unhealthy: ' + ', '.join(c['name'] for c in unhealthy))
        status = STATUS_RUNNING_UNHEALTHY

    # starting: a container (or its health check) is still coming up.
    elif starting or health_starting:
        coming_up = starting + health_starting
        reasons.append('starting: ' + ', '.join(c['name'] for c in coming_up))
        status = STATUS_STARTING

    # running:healthy: everything is up and (if checked) healthy.
    elif len(running) == total:
        status = STATUS_RUNNING_HEALTHY

    # unknown: nothing running and every container is in an unrecognized state.
    elif all(c['state'] == 'unknown' for c in containers):
        reasons.append('container state unknown')
        status = STATUS_UNKNOWN

    # exited: containers exist but none are running.
    elif not running:
        reasons.append('all containers stopped')
        status = STATUS_EXITED

    else:
        status = STATUS_UNKNOWN

    return {
        'status': status,
        'total': total,
        'healthy': len(healthy),
        'reasons': reasons,
        'containers': containers,
    }


def _container_health(container_id):
    """Best-effort health string from docker inspect, or None.

    Reads ``State.Health.Status`` which ``get_container_state`` doesn't expose.
    Defensive: any failure → None (treated as 'no health check').
    """
    from app.services.docker_service import DockerService
    try:
        info = DockerService.get_container(container_id)
        if not info:
            return None
        health = (info.get('State') or {}).get('Health') or {}
        return health.get('Status')
    except Exception:
        return None


def _gather_app_container_states(app):
    """Collect per-container {name, service, state, health} dicts for an app.

    Wraps the Docker CLI calls in try/except so a Docker outage degrades to an
    empty list (→ 'unknown') rather than raising.
    """
    from app.services.docker_service import DockerService
    states = []
    try:
        containers = DockerService.get_all_app_containers(app) or []
    except Exception as e:
        logger.warning('Failed to list containers for app %s: %s',
                       getattr(app, 'id', '?'), e)
        return states

    for c in containers:
        cid = c.get('id') or c.get('name')
        # The container list already carries a state; enrich with health only
        # when the container looks like it's running (cheap inspect avoidance).
        health = None
        if _normalize_state(c.get('state')) == 'running' and cid:
            health = _container_health(cid)
        states.append({
            'id': cid,
            'name': c.get('name'),
            'service': c.get('service'),
            'state': c.get('state'),
            'health': health,
        })
    return states


def get_app_status(application_id, use_cache=True):
    """Aggregated status for a single Application.

    Loads the app, gathers its containers via DockerService, aggregates, and
    caches the result for a short TTL. Fully defensive: a missing app or a
    Docker outage returns a well-formed 'unknown' result rather than raising.

    Returns:
        dict: aggregate_status() output plus 'app_id' and 'kind'.
    """
    from app.services.cache_service import CacheService

    cache_key = f'{_CACHE_PREFIX}:app:{application_id}'
    if use_cache:
        cached = CacheService.get(cache_key)
        if cached is not None:
            return cached

    result = {
        'status': STATUS_UNKNOWN,
        'total': 0,
        'healthy': 0,
        'reasons': [],
        'containers': [],
        'app_id': application_id,
        'kind': 'app',
    }

    try:
        from app.models import Application
        app = Application.query.get(application_id)
    except Exception as e:
        logger.warning('Failed to load application %s: %s', application_id, e)
        app = None

    if not app:
        result['reasons'] = ['application not found']
        return result

    states = _gather_app_container_states(app)
    agg = aggregate_status(states)
    result.update(agg)
    result['app_id'] = application_id
    result['kind'] = 'app'

    if use_cache:
        try:
            CacheService.set(cache_key, result, ttl=_CACHE_TTL)
        except Exception:
            pass
    return result


def get_service_status(service_id, use_cache=True):
    """Aggregated status for a managed service.

    Services in this codebase are modeled as Applications (managed_by /
    compose), so this reuses the app path. Kept as a distinct entry point so
    callers/UI can speak in service terms and so the implementation can diverge
    later without changing the API surface.
    """
    result = get_app_status(service_id, use_cache=use_cache)
    result = dict(result)
    result['kind'] = 'service'
    result['service_id'] = service_id
    return result


def get_database_status(database_id, container_id=None, use_cache=True):
    """Aggregated status for a database.

    Databases don't share the Application container model uniformly. When a
    concrete ``container_id`` is supplied we aggregate it directly; otherwise we
    return a well-formed 'unknown' (best-effort, never raises).
    """
    result = {
        'status': STATUS_UNKNOWN,
        'total': 0,
        'healthy': 0,
        'reasons': ['database container not resolvable'],
        'containers': [],
        'database_id': database_id,
        'kind': 'database',
    }
    if not container_id:
        return result

    try:
        from app.services.docker_service import DockerService
        state = DockerService.get_container_state(container_id)
        health = _container_health(container_id)
        agg = aggregate_status([{
            'id': container_id,
            'name': container_id,
            'state': (state or {}).get('state'),
            'health': health,
        }])
        result.update(agg)
        result['database_id'] = database_id
        result['kind'] = 'database'
    except Exception as e:
        logger.warning('Failed to resolve database %s status: %s', database_id, e)
    return result


def list_app_statuses():
    """Lightweight status summary for every application.

    Returns a list of {app_id, status, total, healthy} suitable for the list
    endpoint and the socket change-detection snapshot. Never raises.
    """
    summaries = []
    try:
        from app.models import Application
        apps = Application.query.all()
    except Exception as e:
        logger.warning('Failed to list applications for status: %s', e)
        return summaries

    for app in apps:
        full = get_app_status(app.id)
        summaries.append({
            'app_id': app.id,
            'status': full.get('status', STATUS_UNKNOWN),
            'total': full.get('total', 0),
            'healthy': full.get('healthy', 0),
        })
    return summaries


# In-memory snapshot of the last emitted statuses, keyed by app_id → status
# string. Used by the socket emitter to emit only on change. Lives in this
# module (single-worker panel) so the emitter stays stateless.
_last_app_statuses = {}


def get_changed_app_statuses():
    """Return only the app statuses that changed since the last call.

    Compares the current per-app aggregated status against the in-memory
    snapshot and updates the snapshot. The socket emitter calls this on its
    interval and emits only the deltas (and drops vanished apps).

    Returns:
        list: changed {app_id, status, total, healthy} summaries.
    """
    changed = []
    current = list_app_statuses()
    seen = set()
    for summary in current:
        app_id = summary['app_id']
        seen.add(app_id)
        if _last_app_statuses.get(app_id) != summary['status']:
            _last_app_statuses[app_id] = summary['status']
            changed.append(summary)

    # Drop apps that disappeared so a re-created id re-emits.
    for gone in [aid for aid in _last_app_statuses if aid not in seen]:
        _last_app_statuses.pop(gone, None)

    return changed
