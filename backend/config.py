import os
import sys
import warnings
from datetime import timedelta

# Default insecure keys that must be changed in production
INSECURE_SECRET_KEYS = [
    'dev-secret-key-change-in-production',
    'jwt-secret-key-change-in-production',
    'change-this-to-a-secure-random-string',
    'change-this-to-another-secure-random-string',
]


def _resolve_ssl_mode():
    """Whether this deployment terminates real end-to-end HTTPS.

    Set by the installer via the ``SERVERKIT_SSL_MODE`` env var or the
    ``/etc/serverkit/ssl-mode`` file. Defaults to ``'insecure'`` so we never
    advertise HSTS/preload on a plain-HTTP or Cloudflare-Flexible deployment —
    that's a hard-to-reverse browser commitment, and HTTPS is intentionally
    optional in ServerKit. Behind a proxy Flask can't tell real TLS from a
    Flexible edge via X-Forwarded-Proto, so we trust the operator's choice.
    """
    mode = os.environ.get('SERVERKIT_SSL_MODE', '').strip().lower()
    if mode in ('secure', 'insecure'):
        return mode
    try:
        with open('/etc/serverkit/ssl-mode', 'r') as fh:
            mode = fh.read().strip().lower()
        if mode in ('secure', 'insecure'):
            return mode
    except OSError:
        pass
    return 'insecure'


class Config:
    SECRET_KEY = os.environ.get('SECRET_KEY', 'dev-secret-key-change-in-production')

    # Database - use instance folder for Flask convention
    SQLALCHEMY_DATABASE_URI = os.environ.get('DATABASE_URL', 'sqlite:////app/instance/serverkit.db')
    SQLALCHEMY_TRACK_MODIFICATIONS = False

    # JWT
    JWT_SECRET_KEY = os.environ.get('JWT_SECRET_KEY', 'jwt-secret-key-change-in-production')
    JWT_ACCESS_TOKEN_EXPIRES = timedelta(minutes=15)
    JWT_REFRESH_TOKEN_EXPIRES = timedelta(days=30)

    # CORS - allow the launcher defaults plus legacy local dev ports.
    DEFAULT_CORS_ORIGINS = ','.join([
        'http://localhost:41921',
        'http://127.0.0.1:41921',
        'http://localhost:47927',
        'http://127.0.0.1:47927',
        'http://localhost:5173',
        'http://127.0.0.1:5173',
        'http://localhost:5000',
        'http://127.0.0.1:5000',
    ])
    _cors_raw = os.environ.get('CORS_ORIGINS', DEFAULT_CORS_ORIGINS)
    CORS_ORIGINS = [o.strip() for o in _cors_raw.split(',') if o.strip()]
    # Whenever the panel sits behind a reverse proxy / tunnel, agents and
    # browsers come from SERVERKIT_PUBLIC_URL — auto-allow it so the user
    # only has to configure that public URL in one place. Without this
    # the engine.io WS handshake rejects the public origin and every
    # Socket.IO connection 400s with "not an accepted origin".
    _public_url = os.environ.get('SERVERKIT_PUBLIC_URL', '').strip().rstrip('/')
    if _public_url and _public_url not in CORS_ORIGINS:
        CORS_ORIGINS.append(_public_url)

    # ── Managed-site routing ────────────────────────────────────────────
    # Each managed site is published at <slug>.<SITES_BASE_DOMAIN>; the
    # operator points a wildcard DNS record (*.<SITES_BASE_DOMAIN>) at the
    # server so new sites are reachable with no per-site DNS work. Empty by
    # default so production must opt in explicitly — when empty, site
    # provisioning falls back to the legacy localhost:<port> URL. The runtime
    # 'sites_base_domain' setting overrides this (see SiteDomainService).
    SITES_BASE_DOMAIN = os.environ.get('SITES_BASE_DOMAIN', '')
    # Public IP the wildcard/custom A-records should point at when auto-creating
    # DNS records via a connected provider.
    SERVER_PUBLIC_IP = os.environ.get('SERVER_PUBLIC_IP', '')

    # Whether the deployment serves real end-to-end HTTPS. Gates the HSTS
    # response header so the panel never forces/preloads SSL on an HTTP-only or
    # Cloudflare-Flexible install (HTTPS is optional). The nginx edge config
    # still emits HSTS in its own secure server block independently of this.
    SSL_MODE = _resolve_ssl_mode()
    HSTS_ENABLED = SSL_MODE == 'secure'

    # ── Build packs ─────────────────────────────────────────────────────
    # Path to the optional nixpacks binary (used by build_service's opaque
    # nixpacks path). The transparent build-pack layer (buildpack_service)
    # generates a Dockerfile instead and does not require this. Workspace dir
    # is where build packs clone/stage sources for detection + generation.
    NIXPACKS_BIN = os.environ.get('NIXPACKS_BIN', 'nixpacks')
    BUILDPACK_WORKSPACE_DIR = os.environ.get(
        'BUILDPACK_WORKSPACE_DIR',
        os.path.join(os.environ.get('SERVERKIT_CACHE_DIR', '/var/cache/serverkit'), 'buildpacks'),
    )


class DevelopmentConfig(Config):
    DEBUG = True
    # lvh.me resolves *.lvh.me -> 127.0.0.1, so subdomain routing works locally
    # with zero DNS setup.
    SITES_BASE_DOMAIN = os.environ.get('SITES_BASE_DOMAIN', 'lvh.me')

    @classmethod
    def init_app(cls, app):
        if app.config.get('SECRET_KEY') == 'dev-secret-key-change-in-production':
            warnings.warn('WARNING: Using default SECRET_KEY. Change before deploying.')
        if app.config.get('JWT_SECRET_KEY') == 'jwt-secret-key-change-in-production':
            warnings.warn('WARNING: Using default JWT_SECRET_KEY. Change before deploying.')


class TestingConfig(Config):
    """Config for pytest and other automated tests."""
    TESTING = True
    DEBUG = True
    SQLALCHEMY_DATABASE_URI = os.environ.get('TEST_DATABASE_URL', 'sqlite:///:memory:')
    # Reduce noise during tests
    JWT_ACCESS_TOKEN_EXPIRES = timedelta(minutes=5)
    # Deterministic base domain so subdomain-provisioning tests don't depend on
    # the developer's shell environment.
    SITES_BASE_DOMAIN = 'lvh.me'


class ProductionConfig(Config):
    DEBUG = False

    # Secure session cookies in production
    SESSION_COOKIE_SECURE = True
    SESSION_COOKIE_HTTPONLY = True
    SESSION_COOKIE_SAMESITE = 'Lax'

    def __init__(self):
        # Validate that secret keys are not default values in production
        if self.SECRET_KEY in INSECURE_SECRET_KEYS:
            print("FATAL: SECRET_KEY is set to a default insecure value in production mode!", file=sys.stderr)
            print("Generate a secure key with: python -c \"import secrets; print(secrets.token_hex(32))\"", file=sys.stderr)
            sys.exit(1)

        if self.JWT_SECRET_KEY in INSECURE_SECRET_KEYS:
            print("FATAL: JWT_SECRET_KEY is set to a default insecure value in production mode!", file=sys.stderr)
            print("Generate a secure key with: python -c \"import secrets; print(secrets.token_hex(32))\"", file=sys.stderr)
            sys.exit(1)

    @classmethod
    def init_app(cls, app):
        """Validate production configuration."""
        insecure_keys = ['dev-secret-key-change-in-production', 'jwt-secret-key-change-in-production']
        if app.config['SECRET_KEY'] in insecure_keys:
            raise ValueError('CRITICAL: SECRET_KEY must be changed for production deployment')
        if app.config['JWT_SECRET_KEY'] in insecure_keys:
            raise ValueError('CRITICAL: JWT_SECRET_KEY must be changed for production deployment')
        # Validate CORS origins
        cors_raw = os.environ.get('CORS_ORIGINS', '')
        cors_origins = [o.strip() for o in cors_raw.split(',') if o.strip()]
        if not cors_origins:
            raise ValueError('CORS_ORIGINS must be explicitly set in production')
        app.config['CORS_ORIGINS'] = cors_origins


config = {
    'development': DevelopmentConfig,
    'testing': TestingConfig,
    'production': ProductionConfig,
    'default': DevelopmentConfig,
}
