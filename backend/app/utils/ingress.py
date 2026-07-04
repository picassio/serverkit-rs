"""Ingress plane — which reverse proxy is expected to serve an app.

ServerKit runs two ingress planes that both bind the public ports (80/443):

  * ``nginx``        — the host Nginx (the default; superior for PHP/WordPress
    /static and Python-behind-gunicorn).
  * ``proxy_stack``  — an opt-in Dockerized proxy (Traefik/Caddy) per server.

A host can only run one of them on 80/443 at a time, so pointing the same
domain at both is an operational footgun. Tagging each app with the plane it
expects keeps the boundary explicit and lets the UI warn when a server's
configured proxy disagrees with the apps running on it.

These helpers are pure (no DB / no I/O) so they are trivially unit-testable and
safe to import anywhere.
"""

INGRESS_NGINX = 'nginx'
INGRESS_PROXY = 'proxy_stack'
INGRESS_PLANES = (INGRESS_NGINX, INGRESS_PROXY)

# Container-based services can be routed by a Dockerized proxy stack. Everything
# else — PHP, WordPress, static sites, Python/gunicorn — is served by the host
# Nginx and is NOT eligible for the proxy stack.
PROXY_ELIGIBLE_APP_TYPES = frozenset({'docker'})


def proxy_eligible(app_type, managed_by=None):
    """True if an app of this type can be served by a Dockerized proxy stack."""
    if (app_type or '').lower() in PROXY_ELIGIBLE_APP_TYPES:
        return True
    return (managed_by or '').lower() == 'docker_compose'


def default_ingress_plane(app_type=None, managed_by=None):
    """Default ingress plane for a new app.

    Always the host Nginx — the proxy stack is strictly opt-in, even for
    eligible (container) apps. This keeps the default create flow on Nginx for
    every app type, as intended.
    """
    return INGRESS_NGINX


def normalize_ingress_plane(value, app_type, managed_by=None):
    """Coerce a requested ingress plane to a valid one for this app type.

    - Unknown / empty -> the default plane.
    - ``proxy_stack`` requested for a non-eligible type -> falls back to
      ``nginx`` (you can't route a PHP/WordPress/static/Python app through a
      container proxy stack here).
    """
    plane = (value or '').lower()
    if plane not in INGRESS_PLANES:
        return default_ingress_plane(app_type, managed_by)
    if plane == INGRESS_PROXY and not proxy_eligible(app_type, managed_by):
        return INGRESS_NGINX
    return plane


def expected_plane_for_proxy(proxy_type):
    """Given a server's active proxy type, the plane its apps should use.

    Host nginx -> ``nginx``; a stack proxy (traefik/caddy) -> ``proxy_stack``.
    """
    return INGRESS_NGINX if (proxy_type or 'nginx').lower() == 'nginx' else INGRESS_PROXY
