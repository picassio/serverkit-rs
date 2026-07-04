"""Domain detection and canonical-domain helpers."""

import re
import ipaddress
from typing import Optional, Tuple
from flask import Request


# Hosts that should never be treated as a canonical production domain.
_LOCAL_HOSTS = {
    'localhost',
    '127.0.0.1',
    '::1',
    '0.0.0.0',
    'lvh.me',
    '*.lvh.me',
}

_DOMAIN_RE = re.compile(
    r'^(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)*'
    r'[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?$'
)


def is_ip_address(host: str) -> bool:
    """Return True if ``host`` is an IPv4 or IPv6 address (with optional port)."""
    if not host:
        return False
    bare = host
    # Strip IPv6 brackets first, e.g. [::1]:8080 -> ::1
    if bare.startswith('['):
        bare = bare.split(']', 1)[0][1:]
    try:
        ipaddress.ip_address(bare)
        return True
    except ValueError:
        pass
    # Could be IPv4:port (e.g. 10.0.0.1:8080). Strip the port and retry.
    if ':' in bare:
        bare = bare.rsplit(':', 1)[0]
        try:
            ipaddress.ip_address(bare)
            return True
        except ValueError:
            pass
    return False


def is_local_host(host: str) -> bool:
    """Return True if ``host`` is localhost or a local/dev wildcard."""
    if not host:
        return True
    host = host.lower().split(':', 1)[0]
    if host in _LOCAL_HOSTS:
        return True
    if host.endswith('.lvh.me') or host.endswith('.local'):
        return True
    return False


def normalize_host(host: str) -> str:
    """Strip port and lower-case a Host header value."""
    if not host:
        return ''
    # IPv6 brackets may contain colons; strip the port outside the brackets.
    if host.startswith('['):
        host = host.split(']', 1)[0] + ']'
    else:
        host = host.split(':', 1)[0]
    return host.lower()


def is_valid_canonical_domain(host: str) -> bool:
    """Return True if ``host`` looks like a real domain we can use as canonical."""
    host = normalize_host(host)
    if not host:
        return False
    if is_ip_address(host):
        return False
    if is_local_host(host):
        return False
    # Reject single-label hosts (e.g., just "serverkit")
    if '.' not in host:
        return False
    return bool(_DOMAIN_RE.match(host))


def detect_request_domain(request: Request) -> Tuple[Optional[str], bool]:
    """Inspect a request and return the candidate canonical domain + HTTPS flag.

    Honors reverse-proxy headers (X-Forwarded-Host, X-Forwarded-Proto) and falls
    back to the request Host / scheme.

    Returns:
        (domain_or_none, is_https)
    """
    forwarded_host = request.headers.get('X-Forwarded-Host', '')
    host = forwarded_host.split(',')[0].strip() or request.host
    host = normalize_host(host)

    forwarded_proto = request.headers.get('X-Forwarded-Proto', '')
    proto = forwarded_proto.split(',')[0].strip() or request.scheme
    is_https = proto.lower() == 'https'

    if is_valid_canonical_domain(host):
        return host, is_https
    return None, is_https


def canonical_origin(domain: str, https_enabled: bool) -> str:
    """Build an origin URL from a canonical domain setting."""
    scheme = 'https' if https_enabled else 'http'
    return f'{scheme}://{domain}'
