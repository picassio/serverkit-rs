"""Server onboarding state machine.

Formalizes "add a server" as an observable lifecycle:

    pending -> validating -> installing_prerequisites -> installing_docker
            -> pairing_agent -> ready    (or -> failed at any step)

Phase 4 scope:
  * A pure :meth:`pre_flight_check` compatibility gate (arch / distro family /
    kernel / CPU / memory) that is fully unit-testable and runs at the top of
    `validate`.
  * `validate` is a real, best-effort transition (server exists, has an
    endpoint or agent, optional reachability probe + pre-flight gate).
  * `install_prerequisites` / `install_docker` / `pair_agent` are REAL but
    heavily GUARDED. On Linux with a package manager / sudo they do real work
    (best-effort); on Windows/dev, or when the host lacks the tooling, they log
    a ``skipped`` detail and succeed. They are idempotent and never raise.
  * Every transition emits an :class:`AuditService` row, and `ready` / `failed`
    fire a best-effort notification.

Each transition appends a `ServerOnboardingLog` row and mirrors a compact
snapshot onto `Server.onboarding_progress` so the wizard can poll cheaply.

In tests / under `ENV=testing` the background job consumer is disabled, so
callers drive the machine synchronously via `start()` / `validate()` /
`advance()`. In production `start()` (and each step) enqueues a
`server.onboarding.advance` job that resumes the machine.
"""
import json
import logging
import os
import re
from datetime import datetime

from flask import current_app, has_app_context

from app import db
from app.models.server import Server
from app.models.server_onboarding_log import ServerOnboardingLog

logger = logging.getLogger(__name__)

# Job kind used to resume / drive the machine in the background.
ONBOARDING_JOB_KIND = 'server.onboarding.advance'

# Number of recent log rows mirrored into Server.onboarding_progress.
_PROGRESS_SNAPSHOT_LIMIT = 40

# Base packages every managed host should carry. Kept small + universally
# available across Debian/RHEL families so the install is unlikely to fail.
_BASE_PACKAGES = ('curl', 'ca-certificates')

# Pre-flight compatibility envelope.
_SUPPORTED_ARCHES = {
    'x86_64': 'x86_64', 'amd64': 'x86_64', 'x64': 'x86_64',
    'arm64': 'arm64', 'aarch64': 'arm64',
}
_SUPPORTED_DISTRO_FAMILIES = {
    'debian', 'ubuntu', 'rhel', 'redhat', 'fedora', 'centos', 'rocky',
    'almalinux', 'alma', 'amazon', 'amzn', 'linux',
}
# Conservative floors. Docker + the agent are light, but these guard against
# obviously-too-small hosts (and stale/embedded kernels).
_MIN_KERNEL = (3, 10)        # RHEL 7 era — Docker's documented floor.
_MIN_CPU_CORES = 1
_MIN_MEMORY_BYTES = 512 * 1024 * 1024  # 512 MiB


class ServerOnboardingService:
    """Drives a server through its provisioning lifecycle."""

    # Lifecycle states, in order. The position in this list defines "next".
    STATE_PENDING = 'pending'
    STATE_VALIDATING = 'validating'
    STATE_INSTALLING_PREREQS = 'installing_prerequisites'
    STATE_INSTALLING_DOCKER = 'installing_docker'
    STATE_PAIRING_AGENT = 'pairing_agent'
    STATE_READY = 'ready'
    STATE_FAILED = 'failed'

    # Ordered active path (excludes the terminal `failed` sink).
    STATES = [
        STATE_PENDING,
        STATE_VALIDATING,
        STATE_INSTALLING_PREREQS,
        STATE_INSTALLING_DOCKER,
        STATE_PAIRING_AGENT,
        STATE_READY,
    ]

    TERMINAL_STATES = (STATE_READY, STATE_FAILED)

    # ------------------------------------------------------------------ #
    # Transition helpers
    # ------------------------------------------------------------------ #

    @classmethod
    def _next_state(cls, state):
        """Return the state that follows ``state`` on the active path, or None
        if there is no successor (already ready / unknown)."""
        try:
            idx = cls.STATES.index(state)
        except ValueError:
            return None
        if idx + 1 < len(cls.STATES):
            return cls.STATES[idx + 1]
        return None

    @classmethod
    def is_valid_transition(cls, from_state, to_state):
        """A transition is valid if it's the immediate next active state, or a
        move to `failed` from any non-terminal state (so any step can fail)."""
        if to_state == cls.STATE_FAILED:
            return from_state not in (cls.STATE_READY,)
        if from_state == to_state:
            return True
        return cls._next_state(from_state) == to_state

    # ------------------------------------------------------------------ #
    # Logging / progress snapshot
    # ------------------------------------------------------------------ #

    @classmethod
    def _log(cls, server, state, status, message=None, detail=None, commit=True):
        """Append an onboarding log row and refresh the cached snapshot."""
        entry = ServerOnboardingLog(
            server_id=server.id,
            state=state,
            status=status,
            message=message,
        )
        entry.set_detail(detail or {})
        db.session.add(entry)
        # Flush so the new row participates in the snapshot query below.
        db.session.flush()
        cls._refresh_snapshot(server)
        if commit:
            db.session.commit()
        return entry

    @classmethod
    def _refresh_snapshot(cls, server):
        """Mirror the most recent log rows onto Server.onboarding_progress."""
        rows = (ServerOnboardingLog.query
                .filter_by(server_id=server.id)
                .order_by(ServerOnboardingLog.created_at.asc(),
                          ServerOnboardingLog.id.asc())
                .limit(_PROGRESS_SNAPSHOT_LIMIT)
                .all())
        snapshot = [r.to_dict() for r in rows]
        try:
            server.onboarding_progress = json.dumps(snapshot)
        except (TypeError, ValueError):
            server.onboarding_progress = '[]'
        server.onboarding_updated_at = datetime.utcnow()

    @classmethod
    def _set_state(cls, server, state):
        server.onboarding_state = state
        server.onboarding_updated_at = datetime.utcnow()

    @classmethod
    def _fail(cls, server, state, message, detail=None):
        """Record a failure on ``state`` and move the machine to `failed`."""
        cls._log(server, state, ServerOnboardingLog.STATUS_FAILED,
                 message=message, detail=detail, commit=False)
        cls._set_state(server, cls.STATE_FAILED)
        db.session.commit()
        cls._audit('server.onboarding.failed', server,
                   {'state': state, 'message': message})
        cls._notify('server.onboarding.failed', server,
                    {'state': state, 'message': message, 'summary': message})

    @classmethod
    def _audit(cls, action, server, details):
        """Best-effort audit; never let an audit failure break onboarding.

        ``AuditLog.target_id`` is an Integer column while ``Server.id`` is a
        UUID string, so we stash the server id inside ``details`` (where it's
        always queryable) and leave ``target_id`` unset.
        """
        try:
            from app.services.audit_service import AuditService
            payload = {'server_id': server.id}
            payload.update(details or {})
            AuditService.log(
                action=action,
                target_type='server',
                details=payload,
            )
        except Exception as exc:  # pragma: no cover - defensive
            logger.debug('onboarding audit %s failed: %s', action, exc)

    @classmethod
    def _notify(cls, event, server, data):
        """Best-effort terminal-state notification. Never raises."""
        try:
            from app.plugins_sdk import notify
            payload = {
                'server': server.name or server.id,
                'server_id': server.id,
                'hostname': server.hostname,
            }
            payload.update(data or {})
            notify.send(event, to='admins', data=payload, category='system')
        except Exception as exc:  # pragma: no cover - defensive
            logger.debug('onboarding notify %s failed: %s', event, exc)

    # ------------------------------------------------------------------ #
    # Pre-flight compatibility check (PURE — fully unit-testable)
    # ------------------------------------------------------------------ #

    @staticmethod
    def _coerce_int(value):
        """Best-effort int from possibly-stringy/None system_info values."""
        if value is None:
            return None
        if isinstance(value, bool):
            return None
        if isinstance(value, (int, float)):
            return int(value)
        try:
            return int(str(value).strip())
        except (TypeError, ValueError):
            return None

    @classmethod
    def _parse_kernel(cls, raw):
        """Return a (major, minor) tuple from a kernel string like
        ``5.15.0-89-generic`` / ``4.18.0-477.el8`` — or None if unparseable."""
        if not raw:
            return None
        m = re.match(r'\s*(\d+)\.(\d+)', str(raw))
        if not m:
            return None
        return (int(m.group(1)), int(m.group(2)))

    @classmethod
    def pre_flight_check(cls, system_info):
        """Pure compatibility gate for a candidate host.

        ``system_info`` is a plain dict (typically derived from the agent's
        capabilities report or the Server row). Recognized keys (all optional):

            architecture / arch        e.g. 'x86_64', 'arm64', 'amd64'
            os_type                    e.g. 'linux', 'windows', 'darwin'
            distro / distro_family /   e.g. 'ubuntu', 'debian', 'rocky'
                os_id / platform
            kernel / kernel_version    e.g. '5.15.0-89-generic'
            cpu_cores                  int
            total_memory               bytes (int)

        Returns::

            {
              'compatible': bool,        # False iff any blocking check failed
              'checks': [{'name','ok','detail'}, ...],
              'blocking': ['<name>', ...],  # subset of failed, hard checks
            }

        A check with ``ok is None`` is "unknown" (info not supplied) — never
        blocking, so a sparse system_info won't wrongly reject a host. Only a
        check that is *known and bad* blocks.
        """
        info = system_info or {}
        checks = []
        blocking = []

        def add(name, ok, detail, hard=True):
            checks.append({'name': name, 'ok': ok, 'detail': detail})
            if hard and ok is False:
                blocking.append(name)

        # --- architecture ---
        raw_arch = (info.get('architecture') or info.get('arch') or '')
        arch = str(raw_arch).strip().lower()
        if not arch:
            add('architecture', None, 'Architecture not reported')
        elif arch in _SUPPORTED_ARCHES:
            add('architecture', True,
                f'{raw_arch} (supported as {_SUPPORTED_ARCHES[arch]})')
        else:
            add('architecture', False,
                f'Unsupported architecture: {raw_arch}')

        # --- OS / distro family ---
        os_type = str(info.get('os_type') or '').strip().lower()
        if os_type and os_type not in ('linux', ''):
            add('os', False,
                f'Unsupported OS: {os_type} (Linux required for provisioning)')
        else:
            distro = str(info.get('distro') or info.get('distro_family')
                         or info.get('os_id') or info.get('platform')
                         or '').strip().lower()
            if not distro:
                add('distro', None, 'Distro not reported')
            else:
                family = distro.split()[0] if distro else ''
                if any(fam in distro for fam in _SUPPORTED_DISTRO_FAMILIES) \
                        or family in _SUPPORTED_DISTRO_FAMILIES:
                    add('distro', True, f'Supported distro family: {distro}')
                else:
                    add('distro', False, f'Unrecognized distro: {distro}')

        # --- kernel ---
        kernel = cls._parse_kernel(info.get('kernel')
                                   or info.get('kernel_version'))
        if kernel is None:
            add('kernel', None, 'Kernel version not reported')
        elif kernel >= _MIN_KERNEL:
            add('kernel', True,
                f'Kernel {kernel[0]}.{kernel[1]} OK '
                f'(>= {_MIN_KERNEL[0]}.{_MIN_KERNEL[1]})')
        else:
            add('kernel', False,
                f'Kernel {kernel[0]}.{kernel[1]} too old '
                f'(need >= {_MIN_KERNEL[0]}.{_MIN_KERNEL[1]})')

        # --- CPU ---
        cores = cls._coerce_int(info.get('cpu_cores'))
        if cores is None:
            add('cpu', None, 'CPU core count not reported')
        elif cores >= _MIN_CPU_CORES:
            add('cpu', True, f'{cores} core(s)')
        else:
            add('cpu', False,
                f'{cores} core(s) (need >= {_MIN_CPU_CORES})')

        # --- memory ---
        mem = cls._coerce_int(info.get('total_memory'))
        if mem is None:
            add('memory', None, 'Total memory not reported')
        elif mem >= _MIN_MEMORY_BYTES:
            add('memory', True,
                f'{mem // (1024 * 1024)} MiB')
        else:
            add('memory', False,
                f'{mem // (1024 * 1024)} MiB '
                f'(need >= {_MIN_MEMORY_BYTES // (1024 * 1024)} MiB)')

        return {
            'compatible': len(blocking) == 0,
            'checks': checks,
            'blocking': blocking,
        }

    @staticmethod
    def _system_info_for(server):
        """Build the pre_flight_check input dict from a Server row.

        Prefers the agent's cached capability snapshot where available, falling
        back to the columns the row already carries.
        """
        caps = server.cached_capabilities or {}
        return {
            'architecture': server.architecture or caps.get('architecture'),
            'os_type': server.os_type or caps.get('os_type'),
            'distro': server.platform or caps.get('distro'),
            'os_id': server.os_version,
            'kernel': caps.get('kernel') or caps.get('kernel_version'),
            'cpu_cores': server.cpu_cores,
            'total_memory': server.total_memory,
        }

    @staticmethod
    def _is_testing():
        if not has_app_context():
            return True
        return bool(current_app.config.get('TESTING') or
                    current_app.config.get('ENV') == 'testing')

    @classmethod
    def _enqueue_advance(cls, server):
        """Schedule a background resume of the machine. No-op under testing
        (callers drive synchronously there)."""
        if cls._is_testing():
            return None
        try:
            from app.plugins_sdk import jobs
            return jobs.enqueue(
                ONBOARDING_JOB_KIND,
                payload={'server_id': server.id},
                owner_type='server',
                owner_id=server.id,
            )
        except Exception as exc:  # pragma: no cover - defensive
            logger.warning('Failed to enqueue onboarding advance for %s: %s',
                           server.id, exc)
            return None

    # ------------------------------------------------------------------ #
    # Public lifecycle entry points
    # ------------------------------------------------------------------ #

    @classmethod
    def start(cls, server_id):
        """Begin onboarding: move pending -> validating and kick off the machine.

        Returns the current status dict. Idempotent-ish: re-calling on an
        in-flight onboarding just re-enqueues an advance.
        """
        server = Server.query.get(server_id)
        if not server:
            raise ValueError(f'Server not found: {server_id}')

        cls._set_state(server, cls.STATE_VALIDATING)
        cls._log(server, cls.STATE_VALIDATING, ServerOnboardingLog.STATUS_STARTED,
                 message='Onboarding started', commit=True)
        cls._audit('server.onboarding.started', server, {'server_id': server.id})

        if cls._is_testing():
            # Synchronous path for tests: run validate immediately so the
            # machine advances without a job consumer.
            cls.validate(server)
        else:
            cls._enqueue_advance(server)
        return cls.get_status(server_id)

    @classmethod
    def validate(cls, server):
        """Best-effort validation that we have enough to provision this server.

        Checks (all soft on dev/Windows):
          * server row exists (caller passes it in)
          * has a hostname/ip OR an already-paired agent
          * records a reachability flag (connected agent counts as reachable)

        On pass: advance to installing_prerequisites. On fail: -> failed.
        """
        if server is None:
            raise ValueError('validate requires a Server instance')

        cls._set_state(server, cls.STATE_VALIDATING)
        cls._log(server, cls.STATE_VALIDATING, ServerOnboardingLog.STATUS_STARTED,
                 message='Validating server details', commit=True)

        has_endpoint = bool((server.hostname or '').strip() or
                            (server.ip_address or '').strip())
        has_agent = bool(server.agent_id)

        reachable = cls._check_reachable(server)

        # Pure compatibility gate — only blocks when the host reports something
        # known-incompatible. A sparse/unknown system_info passes.
        preflight = cls.pre_flight_check(cls._system_info_for(server))

        detail = {
            'has_endpoint': has_endpoint,
            'has_agent': has_agent,
            'reachable': reachable,
            'hostname': server.hostname,
            'ip_address': server.ip_address,
            'preflight': preflight,
        }

        if not has_endpoint and not has_agent:
            cls._fail(
                server, cls.STATE_VALIDATING,
                'No hostname/IP and no paired agent — nothing to connect to.',
                detail=detail,
            )
            return cls.get_status(server.id)

        if not preflight['compatible']:
            cls._fail(
                server, cls.STATE_VALIDATING,
                'Host failed the pre-flight compatibility check: '
                + ', '.join(preflight['blocking']),
                detail=detail,
            )
            return cls.get_status(server.id)

        cls._log(server, cls.STATE_VALIDATING, ServerOnboardingLog.STATUS_SUCCEEDED,
                 message='Validation passed', detail=detail, commit=False)
        cls._set_state(server, cls.STATE_INSTALLING_PREREQS)
        db.session.commit()
        cls._audit('server.onboarding.step', server,
                   {'state': cls.STATE_VALIDATING, 'status': 'succeeded'})

        # Continue down the chain.
        return cls.advance(server)

    @classmethod
    def _check_reachable(cls, server):
        """Defensive reachability probe.

        A connected agent is the strongest signal and works on any OS. We
        deliberately avoid raw ICMP/socket probes here in Phase 1 (they're
        unreliable behind NAT and noisy on dev) — a real network probe lands
        with the install automation phase.
        """
        try:
            from app.services.agent_registry import agent_registry
            if agent_registry.is_agent_connected(server.id):
                return True
        except Exception as exc:  # pragma: no cover - defensive
            logger.debug('reachability check failed for %s: %s', server.id, exc)
        return None  # unknown — not a hard failure in Phase 1

    @classmethod
    def install_prerequisites(cls, server):
        """Install base prerequisites — real on Linux, a safe no-op elsewhere.

        On a Linux host with a detectable package manager we install a small,
        universally-available base set (curl, ca-certificates) via
        ``PackageManager.install``. On Windows/dev, or when no package manager
        is present, we record a ``skipped`` detail and succeed. Idempotent:
        already-installed packages are detected and not reinstalled.
        Never raises.
        """
        done_msg, detail = cls._do_install_prerequisites()
        return cls._run_step(
            server,
            state=cls.STATE_INSTALLING_PREREQS,
            start_msg='Installing prerequisites',
            done_msg=done_msg,
            detail=detail,
        )

    @classmethod
    def _do_install_prerequisites(cls):
        """Best-effort prerequisite install. Returns (done_msg, detail)."""
        from app.utils.system import PackageManager

        if os.name == 'nt':
            return ('Prerequisites skipped (not a Linux host)',
                    {'skipped': True, 'reason': 'windows/dev'})

        try:
            manager = PackageManager.detect()
        except Exception as exc:  # pragma: no cover - defensive
            return ('Prerequisites skipped (package manager probe failed)',
                    {'skipped': True, 'reason': f'detect error: {exc}'})

        if not manager:
            return ('Prerequisites skipped (no supported package manager)',
                    {'skipped': True, 'reason': 'no package manager'})

        # Idempotent: only install what's missing.
        missing = []
        for pkg in _BASE_PACKAGES:
            try:
                if not PackageManager.is_installed(pkg):
                    missing.append(pkg)
            except Exception:  # pragma: no cover - defensive
                missing.append(pkg)

        if not missing:
            return ('Prerequisites already present',
                    {'manager': manager, 'installed': [],
                     'already_present': list(_BASE_PACKAGES)})

        try:
            result = PackageManager.install(list(missing))
            ok = getattr(result, 'returncode', 1) == 0
        except Exception as exc:
            return (f'Prerequisite install best-effort failed: {exc}',
                    {'manager': manager, 'attempted': missing,
                     'best_effort_error': str(exc)})

        if ok:
            return (f'Installed prerequisites: {", ".join(missing)}',
                    {'manager': manager, 'installed': missing})
        return ('Prerequisite install completed with warnings',
                {'manager': manager, 'attempted': missing,
                 'returncode': getattr(result, 'returncode', None)})

    @classmethod
    def install_docker(cls, server):
        """Ensure Docker is available — real on Linux, a safe no-op elsewhere.

        If ``docker`` is already on PATH we record it and succeed. Otherwise on
        a Linux host with sudo/a package manager we make a best-effort install
        attempt (the distro's docker.io / docker package); on Windows/dev or
        without the tooling we record a ``skipped`` detail and succeed so the
        pipeline still completes. Never raises.
        """
        done_msg, detail = cls._do_install_docker()
        return cls._run_step(
            server,
            state=cls.STATE_INSTALLING_DOCKER,
            start_msg='Installing Docker',
            done_msg=done_msg,
            detail=detail,
        )

    @classmethod
    def _do_install_docker(cls):
        """Best-effort Docker install. Returns (done_msg, detail)."""
        from app.utils.system import PackageManager, is_command_available

        docker_present = False
        try:
            docker_present = is_command_available('docker')
        except Exception as exc:  # pragma: no cover - defensive
            logger.debug('docker presence check failed: %s', exc)

        if docker_present:
            return 'Docker already present', {'docker_present': True}

        if os.name == 'nt':
            return ('Docker install skipped (not a Linux host)',
                    {'skipped': True, 'reason': 'windows/dev',
                     'docker_present': False})

        try:
            manager = PackageManager.detect()
        except Exception as exc:  # pragma: no cover - defensive
            manager = None
            logger.debug('docker install pm probe failed: %s', exc)

        if not manager:
            return ('Docker install skipped (no supported package manager)',
                    {'skipped': True, 'reason': 'no package manager',
                     'docker_present': False})

        # Distro-appropriate package name. Debian/Ubuntu ship docker.io; the
        # RHEL family uses the docker package (or podman-docker on minimal
        # images). This is intentionally best-effort — a full
        # get.docker.com convenience-script run lands with agent-driven
        # provisioning; here we just try the distro package so a fresh host
        # isn't left empty-handed.
        pkg = 'docker.io' if manager == 'apt' else 'docker'
        try:
            result = PackageManager.install(pkg)
            ok = getattr(result, 'returncode', 1) == 0
        except Exception as exc:
            return (f'Docker install best-effort failed: {exc}',
                    {'manager': manager, 'attempted': pkg,
                     'best_effort_error': str(exc), 'docker_present': False})

        if ok:
            return (f'Installed Docker ({pkg})',
                    {'manager': manager, 'installed': pkg})
        return ('Docker install completed with warnings',
                {'manager': manager, 'attempted': pkg,
                 'returncode': getattr(result, 'returncode', None)})

    @classmethod
    def pair_agent(cls, server):
        """Pair the management agent — guarded, idempotent, never raises.

        Resolution order:
          * Agent already connected (live socket)  -> satisfied.
          * Server row already carries agent credentials/id -> satisfied
            (registered, may just be momentarily offline).
          * Otherwise -> the step succeeds but records a ``waiting`` detail
            describing what the operator must do (run the install/enroll
            command on the host). We don't hard-fail: a server can be fully
            "ready" in the panel and have its agent connect a moment later.
        """
        done_msg, detail = cls._do_pair_agent(server)
        return cls._run_step(
            server,
            state=cls.STATE_PAIRING_AGENT,
            start_msg='Pairing agent',
            done_msg=done_msg,
            detail=detail,
        )

    @classmethod
    def _do_pair_agent(cls, server):
        """Best-effort agent pairing resolution. Returns (done_msg, detail)."""
        connected = False
        try:
            from app.services.agent_registry import agent_registry
            connected = agent_registry.is_agent_connected(server.id)
        except Exception as exc:  # pragma: no cover - defensive
            logger.debug('agent pairing check failed: %s', exc)

        if connected:
            return 'Agent connected', {'connected': True, 'registered': True}

        # Credentials/identity already minted for this server?
        registered = bool(server.agent_id or server.api_key_hash
                          or server.registration_token_hash)

        if registered:
            return ('Agent registered (awaiting connection)',
                    {'connected': False, 'registered': True,
                     'waiting': 'Agent enrolled but not currently connected.'})

        return ('Agent not yet enrolled',
                {'connected': False, 'registered': False,
                 'waiting': ('Run the agent install/enroll command on the '
                             'host to complete pairing. Onboarding will '
                             'finish; the agent can connect afterward.')})

    @classmethod
    def _run_step(cls, server, state, start_msg, done_msg, detail=None):
        """Shared body for the install/pair steps: log started, log succeeded,
        emit an audit row, advance to the next state, return status."""
        cls._set_state(server, state)
        cls._log(server, state, ServerOnboardingLog.STATUS_STARTED,
                 message=start_msg, commit=True)

        cls._log(server, state, ServerOnboardingLog.STATUS_SUCCEEDED,
                 message=done_msg, detail=detail, commit=False)

        nxt = cls._next_state(state)
        if nxt:
            cls._set_state(server, nxt)
        db.session.commit()
        cls._audit('server.onboarding.step', server,
                   {'state': state, 'status': 'succeeded', 'message': done_msg})
        return cls.advance(server)

    # ------------------------------------------------------------------ #
    # Reconcile / advance / retry
    # ------------------------------------------------------------------ #

    # Dispatch table: which method runs for each current state.
    @classmethod
    def _step_for_state(cls, state):
        return {
            cls.STATE_VALIDATING: cls.validate,
            cls.STATE_INSTALLING_PREREQS: cls.install_prerequisites,
            cls.STATE_INSTALLING_DOCKER: cls.install_docker,
            cls.STATE_PAIRING_AGENT: cls.pair_agent,
        }.get(state)

    @classmethod
    def advance(cls, server):
        """Resume the machine from the server's current onboarding_state.

        Runs the step for the current state, which itself advances to the next
        and recurses, until it reaches a terminal state. Safe to call at any
        point; a no-op on ready/failed/pending.
        """
        if server is None:
            raise ValueError('advance requires a Server instance')

        state = server.onboarding_state or cls.STATE_PENDING

        if state == cls.STATE_READY:
            # Write the terminal "ready" log exactly once (the step that
            # advanced us here only set the state).
            has_ready_log = (ServerOnboardingLog.query
                             .filter_by(server_id=server.id,
                                        state=cls.STATE_READY)
                             .first() is not None)
            if not has_ready_log:
                cls._mark_ready(server)
            return cls.get_status(server.id)

        if state == cls.STATE_FAILED:
            # Don't auto-run on a failed machine; callers use retry().
            return cls.get_status(server.id)

        if state == cls.STATE_PENDING:
            # Not started yet — nothing to advance. start() owns the kickoff.
            return cls.get_status(server.id)

        step = cls._step_for_state(state)
        if step is None:
            return cls.get_status(server.id)

        if state in (cls.STATE_INSTALLING_PREREQS, cls.STATE_INSTALLING_DOCKER,
                     cls.STATE_PAIRING_AGENT):
            # Wrap the install/pair steps so an unexpected error fails
            # gracefully instead of crashing the consumer. The steps are
            # already guarded + never raise on their own; this is belt-and-
            # suspenders for anything truly unforeseen.
            try:
                return step(server)
            except Exception as exc:  # pragma: no cover - defensive
                logger.exception('Onboarding step %s failed for %s', state, server.id)
                cls._fail(server, state, f'Step error: {exc}')
                return cls.get_status(server.id)

        # validate() has its own pass/fail handling.
        return step(server)

    # `reconcile` is an alias spelling of advance for callers that think in
    # reconcile-loop terms.
    @classmethod
    def reconcile(cls, server):
        return cls.advance(server)

    @classmethod
    def _mark_ready(cls, server):
        cls._set_state(server, cls.STATE_READY)
        cls._log(server, cls.STATE_READY, ServerOnboardingLog.STATUS_SUCCEEDED,
                 message='Server ready', commit=True)
        cls._audit('server.onboarding.completed', server, {'server_id': server.id})
        cls._notify('server.onboarding.ready', server,
                    {'summary': f'Server {server.name or server.id} is ready.'})

    @classmethod
    def retry(cls, server_id):
        """Clear a failed onboarding and resume from the start of the pipeline.

        We rewind to `validating` rather than guessing the failed step, so a
        retry always re-checks prerequisites from a clean baseline.
        """
        server = Server.query.get(server_id)
        if not server:
            raise ValueError(f'Server not found: {server_id}')

        if server.onboarding_state != cls.STATE_FAILED:
            # Nothing to recover; just report current status.
            return cls.get_status(server_id)

        cls._set_state(server, cls.STATE_VALIDATING)
        cls._log(server, cls.STATE_VALIDATING, ServerOnboardingLog.STATUS_STARTED,
                 message='Retrying onboarding', commit=True)
        cls._audit('server.onboarding.retried', server, {'server_id': server.id})

        if cls._is_testing():
            cls.validate(server)
        else:
            cls._enqueue_advance(server)
        return cls.get_status(server_id)

    # ------------------------------------------------------------------ #
    # Status read
    # ------------------------------------------------------------------ #

    @classmethod
    def get_status(cls, server_id):
        """Return ``{state, progress: [...logs...], updated_at}`` for a server."""
        server = Server.query.get(server_id)
        if not server:
            raise ValueError(f'Server not found: {server_id}')

        rows = (ServerOnboardingLog.query
                .filter_by(server_id=server_id)
                .order_by(ServerOnboardingLog.created_at.asc(),
                          ServerOnboardingLog.id.asc())
                .all())
        return {
            'server_id': server_id,
            'state': server.onboarding_state or cls.STATE_PENDING,
            'states': cls.STATES,
            'is_terminal': (server.onboarding_state in cls.TERMINAL_STATES),
            'progress': [r.to_dict() for r in rows],
            'updated_at': (server.onboarding_updated_at.isoformat()
                           if server.onboarding_updated_at else None),
        }

    # ------------------------------------------------------------------ #
    # Job registration
    # ------------------------------------------------------------------ #

    @classmethod
    def _advance_job(cls, job):
        """Job handler: resume the machine for the server in the payload."""
        payload = job.get_payload() if hasattr(job, 'get_payload') else (job.payload or {})
        if isinstance(payload, str):
            try:
                payload = json.loads(payload)
            except (TypeError, ValueError):
                payload = {}
        server_id = (payload or {}).get('server_id')
        if not server_id:
            return {'skipped': 'no server_id'}
        server = Server.query.get(server_id)
        if not server:
            return {'skipped': f'server {server_id} not found'}
        status = cls.advance(server)
        return {'state': status.get('state')}

    @classmethod
    def register_jobs(cls):
        """Register the onboarding advance job handler. Call once at app
        startup (wired from app/__init__.py)."""
        from app.jobs import registry
        registry.register(ONBOARDING_JOB_KIND, cls._advance_job, replace=True)
