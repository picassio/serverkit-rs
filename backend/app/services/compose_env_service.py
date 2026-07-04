"""Inject a managed app's effective environment into its Compose services.

Single-container apps get their env via ``docker run -e`` (see
``EnvService.get_effective_env`` + ``DeploymentService._deploy_docker``). Compose
apps instead get container env from the compose file's ``environment:`` block, so
to make shared variable groups + the app's own local env vars reach a compose
app's containers we write a **managed override compose file**
(``docker-compose.serverkit.yml``) next to the app's base compose. The override
adds an ``environment:`` overlay to every service; Docker Compose merges it on top
of the base (override wins on key collisions), so the effective env is
authoritative — matching the single-container behaviour.

This is non-destructive: ServerKit owns only the override file and never edits the
user's / template's base compose. The override is regenerated on every
``compose up`` / ``compose restart`` and removed when the app has no effective env,
so it never goes stale. Everything here is best-effort and guarded — a failure to
build the overlay must never block a deploy; the caller simply falls back to the
plain base compose.
"""
import logging
import os

import yaml

logger = logging.getLogger(__name__)


class ComposeEnvService:
    """Writes/owns the ServerKit env override for a managed compose app."""

    OVERRIDE_NAME = 'docker-compose.serverkit.yml'
    # Conventional base compose filenames, in the order Docker Compose itself
    # prefers them during auto-discovery.
    BASE_CANDIDATES = (
        'docker-compose.yml', 'docker-compose.yaml', 'compose.yml', 'compose.yaml',
    )

    @classmethod
    def find_base_compose(cls, project_path, compose_file=None):
        """Return the base compose file to use (a name/path), or None.

        When ``compose_file`` is given it's authoritative; otherwise discover a
        conventional base file in ``project_path``.
        """
        if compose_file:
            return compose_file
        if not project_path:
            return None
        for name in cls.BASE_CANDIDATES:
            if os.path.exists(os.path.join(project_path, name)):
                return name
        return None

    @classmethod
    def override_path(cls, project_path):
        return os.path.join(project_path, cls.OVERRIDE_NAME)

    @classmethod
    def list_services(cls, app):
        """Service names declared in a managed compose app's base compose.

        Used by the UI to offer a per-service targeting choice. Returns [] for
        non-compose apps or when the base compose can't be read.
        """
        try:
            root = getattr(app, 'root_path', None)
            if not root or not os.path.isdir(root):
                return []
            base = cls.find_base_compose(root, getattr(app, 'compose_file', None))
            if not base:
                return []
            base_path = base if os.path.isabs(base) else os.path.join(root, base)
            return cls._service_names(base_path)
        except Exception:  # pragma: no cover - defensive
            return []

    @staticmethod
    def _app_for_project(project_path):
        """The managed Application whose root_path is this project dir, if any."""
        try:
            from app.models.application import Application
            return Application.query.filter_by(root_path=project_path).first()
        except Exception as e:  # pragma: no cover - DB guard
            logger.debug('compose overlay app lookup failed for %s: %s', project_path, e)
            return None

    @staticmethod
    def _service_names(base_compose_path):
        """Service names declared in the base compose (empty list on any error)."""
        try:
            with open(base_compose_path, 'r', encoding='utf-8') as f:
                data = yaml.safe_load(f) or {}
            services = data.get('services') or {}
            if isinstance(services, dict):
                return list(services.keys())
        except Exception as e:
            logger.debug('could not read services from %s: %s', base_compose_path, e)
        return []

    @staticmethod
    def _escape(value):
        """Escape a value for a compose ``environment:`` entry.

        Compose performs ``${VAR}`` interpolation on environment values, so a
        literal ``$`` must be doubled to survive intact.
        """
        return str(value).replace('$', '$$')

    @classmethod
    def refresh_for_project(cls, project_path, compose_file=None):
        """Regenerate (or remove) the env override for a project dir.

        Returns the absolute override path when one was written, else None.
        Best-effort and fully guarded — never raises.
        """
        try:
            if not project_path or not os.path.isdir(project_path):
                return None

            app = cls._app_for_project(project_path)
            if app is None:
                # Not a managed app dir (e.g. a proxy stack) — leave it alone.
                return None

            base = cls.find_base_compose(project_path, compose_file)
            if not base:
                return None
            base_path = base if os.path.isabs(base) else os.path.join(project_path, base)
            service_names = cls._service_names(base_path)
            if not service_names:
                cls._remove_override(project_path)
                return None

            from app.services.env_service import EnvService
            # Per-service effective env: a variable lands on every service unless
            # it targets a specific one (EnvironmentVariable/SharedVariable
            # .target_service). Local env vars override shared variable groups.
            per_service = EnvService.get_effective_env_for_services(app.id, service_names)
            if not per_service or not any(per_service.values()):
                # No effective env on any service → make sure no stale override lingers.
                cls._remove_override(project_path)
                return None

            services_block = {}
            for name in service_names:
                svc_env = per_service.get(name) or {}
                if not svc_env:
                    continue  # nothing targeted at this service
                services_block[name] = {
                    'environment': {k: cls._escape(v) for k, v in svc_env.items() if k}
                }
            if not services_block:
                cls._remove_override(project_path)
                return None
            override = {'services': services_block}

            override_path = cls.override_path(project_path)
            header = (
                '# Managed by ServerKit — do not edit.\n'
                '# Injects the app\'s effective environment (shared variable groups\n'
                '# under the app\'s own local env vars) into every compose service.\n'
                '# Regenerated on every deploy; delete it and it will be recreated.\n'
            )
            with open(override_path, 'w', encoding='utf-8') as f:
                f.write(header)
                yaml.safe_dump(override, f, default_flow_style=False, sort_keys=True)
            return override_path
        except Exception as e:  # pragma: no cover - defensive
            logger.warning('compose env overlay refresh failed for %s: %s', project_path, e)
            return None

    @classmethod
    def _remove_override(cls, project_path):
        try:
            path = cls.override_path(project_path)
            if os.path.exists(path):
                os.remove(path)
        except OSError as e:  # pragma: no cover - defensive
            logger.debug('could not remove stale compose override: %s', e)
