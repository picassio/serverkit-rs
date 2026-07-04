import os
from flask import Flask, send_from_directory, request, jsonify
from flask_sqlalchemy import SQLAlchemy
from flask_jwt_extended import JWTManager
from flask_cors import CORS
from flask_limiter import Limiter
from flask_limiter.util import get_remote_address
from flask_migrate import Migrate

from config import config

db = SQLAlchemy()
jwt = JWTManager()
migrate = Migrate()

# PyJWT 2.10+ enforces that 'sub' must be a string.
# Stringify the identity so integer user IDs work transparently.
@jwt.user_identity_loader
def _user_identity(user_id):
    return str(user_id)
limiter = Limiter(key_func=get_remote_address, default_limits=["100 per minute"])
# Note: key_func is updated to get_rate_limit_key after app init
socketio = None

# Path to frontend dist folder (relative to backend folder)
FRONTEND_DIST = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(__file__))), 'frontend', 'dist')


def create_app(config_name=None):
    global socketio

    if config_name is None:
        config_name = os.environ.get('FLASK_ENV', 'development')

    # Configure Flask to serve static files from frontend dist
    app = Flask(
        __name__,
        static_folder=FRONTEND_DIST,
        static_url_path=''
    )
    app.config.from_object(config[config_name])

    # Initialize extensions
    db.init_app(app)
    migrate.init_app(app, db)
    jwt.init_app(app)
    limiter.init_app(app)

    # Build CORS origins. Start with static config/env, then append the
    # persisted canonical domain from system settings so pointing an A record
    # at the panel works without restarting to edit .env.
    cors_origins = list(app.config['CORS_ORIGINS'])
    try:
        with app.app_context():
            from app.services.settings_service import SettingsService
            from app.utils.domain import canonical_origin
            canonical_domain = SettingsService.get('canonical_domain', '') or ''
            if canonical_domain:
                https_enabled = SettingsService.get('canonical_https_enabled', False) or False
                origin = canonical_origin(canonical_domain, https_enabled)
                if origin not in cors_origins:
                    cors_origins.append(origin)
    except Exception:
        # Database may not exist yet during first install / migrations.
        pass

    CORS(
        app,
        origins=cors_origins,
        supports_credentials=True,
        allow_headers=['Content-Type', 'Authorization', 'X-Requested-With', 'X-API-Key'],
        methods=['GET', 'POST', 'PUT', 'DELETE', 'OPTIONS', 'PATCH']
    )

    # Register security headers middleware
    from app.middleware.security import register_security_headers
    register_security_headers(app)

    # Register API key authentication middleware
    from app.middleware.api_key_auth import register_api_key_auth
    register_api_key_auth(app)

    # Register API analytics middleware
    from app.middleware.api_analytics import register_api_analytics
    register_api_analytics(app)

    # Register fallback audit logging for authenticated mutating API requests
    from app.middleware.audit import register_audit_fallback
    register_audit_fallback(app)

    # Update rate limiter with custom key function
    from app.middleware.rate_limit import get_rate_limit_key, register_rate_limit_headers
    limiter._key_func = get_rate_limit_key
    register_rate_limit_headers(app)

    # Initialize SocketIO
    from app.sockets import init_socketio
    socketio = init_socketio(app)

    # Initialize Agent Gateway
    from app.agent_gateway import init_agent_gateway
    init_agent_gateway(socketio)

    # Register blueprints - Auth
    from app.api.auth import auth_bp
    app.register_blueprint(auth_bp, url_prefix='/api/v1/auth')

    # Agent polling fallback transport (REST equivalent of the WS gateway,
    # used when tunnels mangle WebSocket frames).
    from app.api.agent_poll import agent_poll_bp
    app.register_blueprint(agent_poll_bp, url_prefix='/api/v1/agent')

    # Register blueprints - Core
    from app.api.apps import apps_bp
    from app.api.domains import domains_bp
    from app.api.tunnels import tunnels_bp
    from app.api.private_urls import private_urls_bp
    app.register_blueprint(apps_bp, url_prefix='/api/v1/apps')
    # "Services" is the user-facing term for Applications (§1 unification).
    # Mount the same blueprint under /api/v1/services as a true alias so the
    # canonical `apps` routes and any `services` callers resolve identically.
    app.register_blueprint(apps_bp, url_prefix='/api/v1/services', name='services')
    app.register_blueprint(domains_bp, url_prefix='/api/v1/domains')
    app.register_blueprint(private_urls_bp, url_prefix='/api/v1/apps')
    # Per-app managed volumes — same dual mount as apps (/apps + /services alias).
    from app.api.app_volumes import app_volumes_bp
    app.register_blueprint(app_volumes_bp, url_prefix='/api/v1/apps')
    app.register_blueprint(app_volumes_bp, url_prefix='/api/v1/services', name='app_volumes_services')

    # Register blueprints - System
    from app.api.system import system_bp
    from app.api.processes import processes_bp
    from app.api.logs import logs_bp
    app.register_blueprint(system_bp, url_prefix='/api/v1/system')
    app.register_blueprint(processes_bp, url_prefix='/api/v1/processes')
    app.register_blueprint(logs_bp, url_prefix='/api/v1/logs')

    # Register blueprints - Infrastructure
    from app.api.nginx import nginx_bp
    from app.api.ssl import ssl_bp
    app.register_blueprint(nginx_bp, url_prefix='/api/v1/nginx')
    app.register_blueprint(ssl_bp, url_prefix='/api/v1/ssl')

    # Register blueprints - PHP
    from app.api.php import php_bp
    app.register_blueprint(php_bp, url_prefix='/api/v1/php')
    # WordPress moved into the bundled, default-installed `serverkit-wordpress`
    # extension (#38). Its blueprints (wordpress / wordpress_sites /
    # environment_pipeline, keeping the /api/v1/wordpress[/projects|/pipelines]
    # prefixes per D9, incl. the /pipelines alias) are registered from the
    # extension by the plugin loader — seeded as a flagship in create_app. The old
    # `wordpress` module toggle is retired; the plugin status guard is the gate.

    # Register blueprints - Python
    from app.api.python import python_bp
    app.register_blueprint(python_bp, url_prefix='/api/v1/python')

    # Register blueprints - Docker
    from app.api.docker import docker_bp
    app.register_blueprint(docker_bp, url_prefix='/api/v1/docker')

    # Register blueprints - Databases
    from app.api.databases import databases_bp
    app.register_blueprint(databases_bp, url_prefix='/api/v1/databases')

    # Register blueprints - Monitoring & Alerts
    from app.api.monitoring import monitoring_bp
    app.register_blueprint(monitoring_bp, url_prefix='/api/v1/monitoring')

    # Register blueprints - Container status aggregator
    from app.api.container_status import container_status_bp
    app.register_blueprint(container_status_bp, url_prefix='/api/v1/status')

    # Register blueprints - Build packs (zero-Dockerfile detection)
    from app.api.buildpacks import buildpacks_bp
    app.register_blueprint(buildpacks_bp, url_prefix='/api/v1/buildpacks')

    # Register blueprints - Deployment config snapshots + diff
    from app.api.snapshots import snapshots_bp
    app.register_blueprint(snapshots_bp, url_prefix='/api/v1/apps')

    # Register blueprints - Projects & Environments hierarchy
    from app.api.projects import projects_bp
    app.register_blueprint(projects_bp, url_prefix='/api/v1/projects')
    from app.api.environments import environments_bp
    app.register_blueprint(environments_bp, url_prefix='/api/v1/environments')

    # Register blueprints - Polymorphic shared resources (tags + variable groups)
    from app.api.shared_resources import shared_resources_bp
    app.register_blueprint(shared_resources_bp, url_prefix='/api/v1/shared')

    # Register blueprints - PR preview environments
    from app.api.previews import previews_bp
    app.register_blueprint(previews_bp, url_prefix='/api/v1/apps')
    from app.api.webhooks import webhooks_bp
    app.register_blueprint(webhooks_bp, url_prefix='/api/v1/webhooks')

    # Register blueprints - Per-server managed proxy stack
    from app.api.proxy import proxy_bp
    app.register_blueprint(proxy_bp, url_prefix='/api/v1/servers')

    # Register blueprints - Notifications
    from app.api.notifications import notifications_bp
    app.register_blueprint(notifications_bp, url_prefix='/api/v1/notifications')

    # Register blueprints - Backups
    from app.api.backups import backups_bp
    app.register_blueprint(backups_bp, url_prefix='/api/v1/backups')

    # Register blueprints - Git Deployment
    from app.api.deploy import deploy_bp
    app.register_blueprint(deploy_bp, url_prefix='/api/v1/deploy')

    # Register blueprints - Builds & Deployments
    from app.api.builds import builds_bp
    from app.api.deployment_jobs import deployment_jobs_bp
    from app.api.deployments import deployments_bp
    app.register_blueprint(builds_bp, url_prefix='/api/v1/builds')
    app.register_blueprint(deployment_jobs_bp, url_prefix='/api/v1/deployment-jobs')
    # §3 unification: one /api/v1/deployments surface. Federated history/detail
    # live in deployments_bp; the canonical execution records (DeploymentJob)
    # are also mounted here under /deployments/jobs (alias of /deployment-jobs).
    app.register_blueprint(deployments_bp, url_prefix='/api/v1/deployments')
    app.register_blueprint(deployment_jobs_bp, url_prefix='/api/v1/deployments/jobs',
                           name='deployment_jobs_unified')

    # Register blueprints - Templates
    from app.api.templates import templates_bp
    app.register_blueprint(templates_bp, url_prefix='/api/v1/templates')

    # Register blueprints - File Manager
    from app.api.files import files_bp
    app.register_blueprint(files_bp, url_prefix='/api/v1/files')

    # Register blueprints - FTP Server
    from app.api.ftp import ftp_bp
    app.register_blueprint(ftp_bp, url_prefix='/api/v1/ftp')

    # Register blueprints - Firewall
    from app.api.firewall import firewall_bp
    app.register_blueprint(firewall_bp, url_prefix='/api/v1/firewall')

    # Register blueprints - Git Server
    from app.api.git import git_bp
    app.register_blueprint(git_bp, url_prefix='/api/v1/git')

    # Register blueprints - Security (ClamAV, File Integrity, etc.)
    from app.api.security import security_bp
    app.register_blueprint(security_bp, url_prefix='/api/v1/security')

    # Register blueprints - Secrets manager + inbound webhook gateway
    from app.api.secrets_webhooks import bp as secrets_webhooks_bp
    app.register_blueprint(secrets_webhooks_bp, url_prefix='/api/v1')

    # Register blueprints - Cron Jobs
    from app.api.cron import cron_bp
    app.register_blueprint(cron_bp, url_prefix='/api/v1/cron')

    # Email Server is now the serverkit-email builtin extension (Phase 4, #32):
    # its /api/v1/email blueprint + Postfix/Dovecot/DKIM/SpamAssassin/Roundcube
    # services live in builtin-extensions/serverkit-email/ and are registered by
    # the plugin loader when installed. The outbound relay (email_relay_service)
    # and all email models stay core (notifications SMTP + shared Postfix relay).

    # Register blueprints - Uptime Tracking
    from app.api.uptime import uptime_bp
    app.register_blueprint(uptime_bp, url_prefix='/api/v1/uptime')

    # Register blueprints - Environment Variables
    from app.api.env_vars import env_vars_bp
    app.register_blueprint(env_vars_bp, url_prefix='/api/v1/apps')

    # Register blueprints - Two-Factor Authentication
    from app.api.two_factor import two_factor_bp
    app.register_blueprint(two_factor_bp, url_prefix='/api/v1/auth/2fa')

    # Register blueprints - SSO / OAuth
    from app.api.sso import sso_bp
    app.register_blueprint(sso_bp, url_prefix='/api/v1/sso')

    # Register blueprints - Source provider connections
    from app.api.source_connections import source_connections_bp
    app.register_blueprint(source_connections_bp, url_prefix='/api/v1/source-connections')

    # Register blueprints - Domain registrar connections (portfolio + expiry)
    from app.api.registrars import registrars_bp
    app.register_blueprint(registrars_bp, url_prefix='/api/v1/registrars')

    # Register blueprints - Unified connection registry (read-only "all connections")
    from app.api.connections import connections_bp
    app.register_blueprint(connections_bp, url_prefix='/api/v1/connections')

    # Register blueprints - Database Migrations
    from app.api.migrations import migrations_bp
    app.register_blueprint(migrations_bp, url_prefix='/api/v1/migrations')

    # Register blueprints - API Enhancements
    from app.api.api_keys import api_keys_bp
    from app.api.api_analytics import api_analytics_bp
    from app.api.event_subscriptions import event_subscriptions_bp
    from app.api.docs import docs_bp
    app.register_blueprint(api_keys_bp, url_prefix='/api/v1/api-keys')
    app.register_blueprint(api_analytics_bp, url_prefix='/api/v1/api-analytics')
    app.register_blueprint(event_subscriptions_bp, url_prefix='/api/v1/event-subscriptions')
    app.register_blueprint(docs_bp, url_prefix='/api/v1/docs')

    # Register blueprints - Admin (User Management, Settings, Audit Logs)
    from app.api.admin import admin_bp
    app.register_blueprint(admin_bp, url_prefix='/api/v1/admin')

    # Register blueprints - Invitations
    from app.api.invitations import invitations_bp
    app.register_blueprint(invitations_bp, url_prefix='/api/v1/admin/invitations')

    # Register blueprints - Historical Metrics
    from app.api.metrics import metrics_bp
    app.register_blueprint(metrics_bp, url_prefix='/api/v1/metrics')

    # Register blueprints - Workflows
    from app.api.workflows import workflows_bp
    app.register_blueprint(workflows_bp, url_prefix='/api/v1/workflows')

    # Register blueprints - Servers (Multi-server management)
    from app.api.servers import servers_bp
    app.register_blueprint(servers_bp, url_prefix='/api/v1/servers')

    # Register blueprints - Fleet Monitor (Cross-server monitoring)
    from app.api.fleet_monitor import fleet_monitor_bp
    app.register_blueprint(fleet_monitor_bp, url_prefix='/api/v1/fleet-monitor')

    # Register blueprints - Fleet (target picker, capability discovery)
    from app.api.fleet import fleet_bp
    app.register_blueprint(fleet_bp, url_prefix='/api/v1/fleet')

    # Register blueprints - Agent Plugins
    from app.api.agent_plugins import agent_plugins_bp
    app.register_blueprint(agent_plugins_bp, url_prefix='/api/v1/agent-plugins')

    # Register blueprints - Server Templates
    from app.api.server_templates import server_templates_bp
    app.register_blueprint(server_templates_bp, url_prefix='/api/v1/server-templates')

    # Register blueprints - Workspaces
    from app.api.workspaces import workspaces_bp
    app.register_blueprint(workspaces_bp, url_prefix='/api/v1/workspaces')

    # Register blueprints - Advanced SSL
    # §5 unification: one SSL surface. The advanced cert operations (wildcard,
    # SAN, custom upload, profiles, health, expiry alerts) mount under the same
    # /api/v1/ssl prefix as the basic certbot routes (no path collisions). The
    # original /api/v1/ssl/advanced prefix is kept as a deprecated alias.
    from app.api.advanced_ssl import advanced_ssl_bp
    app.register_blueprint(advanced_ssl_bp, url_prefix='/api/v1/ssl/advanced')
    app.register_blueprint(advanced_ssl_bp, url_prefix='/api/v1/ssl', name='advanced_ssl_unified')

    # Register blueprints - DNS Zones
    from app.api.dns_zones import dns_zones_bp
    app.register_blueprint(dns_zones_bp, url_prefix='/api/v1/dns')

    # Register blueprints - Cloudflare operations (zone settings/cache/WAF on top
    # of the existing Cloudflare DNS connection)
    # Cloudflare zone-ops moved into the bundled, default-installed
    # `serverkit-cloudflare-ops` extension (#36). Its blueprint (kept at
    # /api/v1/cloudflare, D9) is registered from the extension by the plugin
    # loader — seeded as a flagship in create_app. DNS records + the Cloudflare
    # connection stay core (they back /domains); the extension borrows the single
    # core CloudflareClient, never a duplicate.

    # Register blueprints - Dynamic DNS
    from app.api.ddns import ddns_bp
    app.register_blueprint(ddns_bp, url_prefix='/api/v1/ddns')

    # Register blueprints - Image update checks
    from app.api.image_updates import image_updates_bp
    app.register_blueprint(image_updates_bp, url_prefix='/api/v1/image-updates')

    # Register blueprints - Per-application WAF (ModSecurity + OWASP CRS)
    from app.api.waf import waf_bp
    app.register_blueprint(waf_bp, url_prefix='/api/v1/waf')

    # Register blueprints - GPU monitoring
    from app.api.gpu import gpu_bp
    app.register_blueprint(gpu_bp, url_prefix='/api/v1/gpu')

    # Register blueprints - Nginx Advanced
    from app.api.nginx_advanced import nginx_advanced_bp
    app.register_blueprint(nginx_advanced_bp, url_prefix='/api/v1/nginx/advanced')

    # Register blueprints - Status Pages
    from app.api.status_pages import status_pages_bp
    app.register_blueprint(status_pages_bp, url_prefix='/api/v1/status')

    # Register blueprints - Cloud Provisioning
    from app.api.cloud_provisioning import cloud_provisioning_bp
    app.register_blueprint(cloud_provisioning_bp, url_prefix='/api/v1/cloud')

    # Register blueprints - Remote Access (WireGuard tunnels; imported above)
    app.register_blueprint(tunnels_bp, url_prefix='/api/v1/tunnels')

    # Register blueprints - Performance
    from app.api.performance import performance_bp
    app.register_blueprint(performance_bp, url_prefix='/api/v1/performance')

    # Register blueprints - Mobile
    from app.api.mobile import mobile_bp
    app.register_blueprint(mobile_bp, url_prefix='/api/v1/mobile')

    # Register blueprints - Marketplace
    from app.api.marketplace import marketplace_bp
    app.register_blueprint(marketplace_bp, url_prefix='/api/v1/marketplace')

    # Register blueprints - Plugins
    from app.api.plugins import plugins_bp
    app.register_blueprint(plugins_bp, url_prefix='/api/v1/plugins')

    # Register blueprints - Modules (core-vertical toggles)
    from app.api.modules import modules_bp
    app.register_blueprint(modules_bp, url_prefix='/api/v1/modules')

    # Register blueprints - Queue Bus
    from app.api.queue_bus import queue_bus_bp
    app.register_blueprint(queue_bus_bp, url_prefix='/api/v1/queue')

    # Register blueprints - Unified Jobs (work orchestration on the Queue Bus)
    from app.api.jobs import jobs_bp
    app.register_blueprint(jobs_bp, url_prefix='/api/v1/jobs')

    # Register blueprints - Telemetry / System Event Stream
    from app.api.telemetry import telemetry_bp
    app.register_blueprint(telemetry_bp, url_prefix='/api/v1/telemetry')

    # §4 unification: one observability namespace. The monitoring / metrics /
    # telemetry / uptime / fleet / status-page read surfaces are re-mounted under
    # /api/v1/observability/<domain> as true aliases (same blueprints, distinct
    # names) so callers have a single front door. The original prefixes remain,
    # and the PUBLIC status page route (/api/v1/status/public/<slug>) is
    # unchanged — its canonical mount is untouched.
    app.register_blueprint(monitoring_bp, url_prefix='/api/v1/observability/monitoring', name='obs_monitoring')
    app.register_blueprint(metrics_bp, url_prefix='/api/v1/observability/metrics', name='obs_metrics')
    app.register_blueprint(telemetry_bp, url_prefix='/api/v1/observability/events', name='obs_events')
    app.register_blueprint(uptime_bp, url_prefix='/api/v1/observability/uptime', name='obs_uptime')
    app.register_blueprint(fleet_monitor_bp, url_prefix='/api/v1/observability/fleet', name='obs_fleet')
    app.register_blueprint(status_pages_bp, url_prefix='/api/v1/observability/status-pages', name='obs_status_pages')

    # Register blueprints - Agent Pairing (RustDesk-style short-code flow)
    from app.api.pairing import pairing_bp
    app.register_blueprint(pairing_bp, url_prefix='/api/v1/pairing')

    # Register blueprints - AI Assistant (core primitive, powered by Prompture)
    from app.api.ai import ai_bp
    app.register_blueprint(ai_bp, url_prefix='/api/v1/ai')

    # Handle database migrations (Alembic) — must run before plugin loader
    # since the loader queries the installed_plugins table.
    with app.app_context():
        from app.services.migration_service import MigrationService
        MigrationService.check_and_prepare(app)

        # Initialize default settings and migrate legacy roles
        from app.services.settings_service import SettingsService
        SettingsService.initialize_defaults()
        SettingsService.migrate_legacy_roles()

        # Encrypt any legacy plaintext provider secrets at rest (idempotent —
        # DNS-provider api keys and storage credentials predate encryption).
        try:
            from app.services.dns_provider_service import DNSProviderService
            from app.services.storage_provider_service import StorageProviderService
            from app.services.cloud_provisioning_service import CloudProvisioningService
            n_dns = DNSProviderService.encrypt_legacy_secrets()
            n_store = StorageProviderService.encrypt_legacy_secrets()
            n_cloud = CloudProvisioningService.encrypt_legacy_secrets()
            n_settings = SettingsService.migrate_legacy_secrets()
            # Migrate DNS zones with an inline Cloudflare token onto the canonical
            # connection store (idempotent), so every zone resolves creds the same way.
            from app.services.dns_zone_service import DNSZoneService
            n_zones = DNSZoneService.link_legacy_zones()
            # Fold a legacy single-row email relay config into the unified
            # EmailProviderConnection table (§6); idempotent, best-effort.
            try:
                from app.services.email_relay_service import EmailRelayService
                EmailRelayService.migrate_legacy_config()
            except Exception as _relay_exc:  # never block boot on this
                import logging as _logging
                _logging.getLogger(__name__).warning(
                    f'Email relay legacy migration skipped: {_relay_exc}')
            if n_dns or n_store or n_cloud or n_settings or n_zones:
                import logging as _logging
                _logging.getLogger(__name__).info(
                    f'Encrypted legacy secrets at rest: {n_dns} DNS provider(s), '
                    f'{n_store} storage field(s), {n_cloud} cloud provider(s), '
                    f'{n_settings} system setting(s); linked {n_zones} DNS zone(s) '
                    f'to a connection')
        except Exception as e:
            import logging as _logging
            _logging.getLogger(__name__).warning(f'Legacy secret encryption skipped: {e}')

        # Seed bundled flagship extensions (D4) — WordPress ships installed by
        # default on every panel (fresh and upgrade) unless the user uninstalled
        # it. Done BEFORE load_all_plugins so the loader registers the seeded
        # blueprints. In-place: no file copy. Best-effort.
        try:
            from app.services.plugin_service import seed_flagship_extensions
            seed_flagship_extensions()
        except Exception as e:
            import logging
            logging.getLogger(__name__).warning(f'Flagship seed: {e}')

        # Load installed plugins (dynamic blueprints) AFTER migrations,
        # so the installed_plugins table exists.
        try:
            from app.services.plugin_service import load_all_plugins
            load_all_plugins(app)
        except Exception as e:
            import logging
            logging.getLogger(__name__).warning(f'Plugin loader: {e}')

        # One-shot: auto-install builtin extensions that used to be core pages
        # so an upgraded panel doesn't lose the feature (decision D3). Fresh
        # installs see them in the Marketplace instead. Best-effort.
        try:
            from app.services.extension_migration import run_auto_install
            run_auto_install()
        except Exception as e:
            import logging
            logging.getLogger(__name__).warning(f'Extension auto-install: {e}')

        # Start metrics history collection in background
        from app.services.metrics_history_service import MetricsHistoryService
        if not MetricsHistoryService.is_running():
            MetricsHistoryService.start_collection(app)

        # Start queue-bus webhook consumer
        from app.queue_bus.consumers import start_webhook_consumer
        start_webhook_consumer(app)

        # Start queue-bus notification consumer (delivers in-app/email/chat)
        from app.notifications.consumer import start_notification_consumer
        start_notification_consumer(app)

        # Start the API analytics flush thread (a 5s buffer flush — a real-time
        # stream, deliberately NOT modeled as a job).
        from app.middleware.api_analytics import start_analytics_flush_thread
        start_analytics_flush_thread(app)

        # Start the unified job system: ONE consumer runs every enqueued Job and
        # ONE scheduler ticks all periodic work. This supersedes the former set
        # of per-domain daemon scheduler threads (auto-sync, snapshot-retention,
        # workflow, health-check, wp-update, api-background, pairing-prune,
        # registrar-expiry) — they are now ScheduledJob rows backed by the
        # built-in handlers in app/jobs/builtin_handlers.py.
        from app.jobs import start_job_system
        from app.jobs.builtin_handlers import register_builtin_handlers, seed_builtin_schedules
        register_builtin_handlers()
        # Register event-driven job handlers (deployment installs, workflow runs,
        # scheduled backups).
        from app.services.deployment_job_service import DeploymentJobService
        DeploymentJobService.register_jobs()
        from app.services.workflow_engine import WorkflowEngine
        WorkflowEngine.register_jobs()
        from app.services.backup_service import BackupService
        BackupService.register_jobs()
        from app.services.backup_policy_service import BackupPolicyService
        BackupPolicyService.register_jobs()
        from app.services.server_onboarding_service import ServerOnboardingService
        ServerOnboardingService.register_jobs()
        from app.services.preview_service import PreviewService
        PreviewService.register_jobs()
        start_job_system(app, seed=seed_builtin_schedules)

    # Request body size limit
    app.config['MAX_CONTENT_LENGTH'] = 100 * 1024 * 1024  # 100MB limit

    # Reject 2FA pending tokens on non-2FA endpoints
    @app.before_request
    def check_2fa_pending():
        """Reject 2FA pending tokens on non-2FA endpoints."""
        from flask_jwt_extended import verify_jwt_in_request, get_jwt
        if request.endpoint and request.path.startswith('/api/'):
            # Allow 2FA verification endpoints
            if '/two-factor/verify' in request.path or '/two-factor/verify-backup' in request.path:
                return
            # Allow auth endpoints (login, refresh)
            if '/auth/login' in request.path or '/auth/refresh' in request.path:
                return
            try:
                verify_jwt_in_request()
                claims = get_jwt()
                if claims.get('2fa_pending'):
                    return jsonify({'error': '2FA verification required'}), 403
            except Exception:
                pass  # Let @jwt_required handle actual auth errors

    # Serve frontend for root path
    @app.route('/')
    def serve_index():
        index = os.path.join(app.static_folder, 'index.html') if app.static_folder else None
        if index and os.path.isfile(index):
            return send_from_directory(app.static_folder, 'index.html')
        return {'message': 'ServerKit API is running', 'docs': '/api/v1/'}, 200

    # Catch-all route for SPA - must be after all other routes
    @app.errorhandler(404)
    def not_found(e):
        from flask import request
        if request.path.startswith('/api/'):
            return {'error': 'Not found'}, 404
        # Serve SPA index.html if it exists, otherwise JSON 404
        index = os.path.join(app.static_folder, 'index.html') if app.static_folder else None
        if index and os.path.isfile(index):
            return send_from_directory(app.static_folder, 'index.html')
        return {'error': 'Not found'}, 404

    return app


def get_socketio():
    """Get the SocketIO instance."""
    return socketio
