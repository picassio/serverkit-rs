"""Pure, dependency-free helpers for the WireGuard tunnel broker.

Kept import-light (stdlib only) so the subnet allocator, interface-name
derivation and health classification are unit-testable without booting
Flask/SQLAlchemy. See docs/REMOTE_ACCESS_ROADMAP.md (Phase 1, #7 / #11).
"""

import ipaddress

# The tunnel overlay pool: 10.88.0.0/16 carved into /24s. Deliberately an
# uncommon RFC-1918 block so it won't collide with a typical home LAN
# (192.168.x / 10.0.x). Each tunnel gets one /24 — .1 = edge, .2 = private.
TUNNEL_POOL = "10.88.0.0/16"

# A peer counts as "live" if it handshook within this window. WireGuard
# rehandshakes well inside it when persistent-keepalive is 25s.
HANDSHAKE_FRESH_SECONDS = 180

# WireGuard listen ports are allocated per edge (#20) so ONE edge can front
# many private peers — each tunnel needs its own UDP port. 51820..52019.
DEFAULT_LISTEN_PORT = 51820
LISTEN_PORT_RANGE = 200

# A tunnel with both interfaces up but no handshake after this long likely
# has its outbound UDP blocked (#21).
HANDSHAKE_GRACE_SECONDS = 45


def pick_subnet(used_subnets):
    """Return (subnet_cidr, edge_ip, private_ip) for the first free /24 in
    the pool. ``used_subnets`` is any iterable of CIDR strings already taken.

    Raises RuntimeError if the pool is exhausted (256 /24s).
    """
    used = set(used_subnets or [])
    for third in range(256):
        cidr = "10.88.%d.0/24" % third
        if cidr not in used:
            return cidr, "10.88.%d.1" % third, "10.88.%d.2" % third
    raise RuntimeError("tunnel subnet pool exhausted (10.88.0.0/16, 256 /24s)")


def interface_name_for(tunnel_id):
    """Derive a stable, kernel-valid WireGuard interface name from a tunnel
    id. Linux caps interface names at 15 chars; 'skwg' + 8 hex is 12. The
    same name is used on both ends (they're different hosts).
    """
    compact = str(tunnel_id).replace("-", "")[:8] or "0"
    return "skwg%s" % compact


def derive_status(latest_handshake_epoch, now_epoch, interface_up=True):
    """Classify tunnel health from a peer's latest-handshake timestamp.

    - 'down'     — the interface isn't up
    - 'up'       — handshook within HANDSHAKE_FRESH_SECONDS
    - 'degraded' — handshook before, but now stale (link may be recovering)
    - 'pending'  — interface up but no handshake yet (just created)
    """
    if not interface_up:
        return "down"
    if latest_handshake_epoch and latest_handshake_epoch > 0:
        age = now_epoch - latest_handshake_epoch
        return "up" if age <= HANDSHAKE_FRESH_SECONDS else "degraded"
    return "pending"


def validate_endpoint_host(ip_address):
    """True if ``ip_address`` is usable as the edge's public endpoint host.

    Accepts IPv4/IPv6; rejects empty, malformed, loopback and unspecified
    addresses. Private ranges are allowed (a lab/LAN edge is valid).
    """
    if not ip_address:
        return False
    try:
        ip = ipaddress.ip_address(str(ip_address).strip())
    except ValueError:
        return False
    return not (ip.is_loopback or ip.is_unspecified)


def pick_listen_port(used_ports):
    """First free WireGuard listen port for an edge, starting at
    DEFAULT_LISTEN_PORT. ``used_ports`` is the ports already taken by other
    tunnels on the SAME edge — so one edge can front many private peers (#20).
    Raises RuntimeError if the per-edge pool is exhausted.
    """
    used = set(used_ports or [])
    for port in range(DEFAULT_LISTEN_PORT, DEFAULT_LISTEN_PORT + LISTEN_PORT_RANGE):
        if port not in used:
            return port
    raise RuntimeError(
        "listen-port pool exhausted on this edge (%d..%d)"
        % (DEFAULT_LISTEN_PORT, DEFAULT_LISTEN_PORT + LISTEN_PORT_RANGE - 1)
    )


def diagnose_reachability(latest_handshake_epoch, age_seconds, both_interfaces_up):
    """Near-term #21: classify why a tunnel isn't passing traffic, with an
    actionable hint. The actual relay data plane is deferred.

    - 'interface_down' — an agent hasn't brought its interface up
    - 'ok'             — a handshake has happened
    - 'connecting'     — interfaces up, still within the handshake grace window
    - 'no_handshake'   — up + past grace + no handshake → outbound UDP likely blocked
    """
    if not both_interfaces_up:
        return {'state': 'interface_down',
                'hint': 'One or both agents have not brought the WireGuard interface up.'}
    if latest_handshake_epoch and latest_handshake_epoch > 0:
        return {'state': 'ok'}
    if age_seconds is not None and age_seconds < HANDSHAKE_GRACE_SECONDS:
        return {'state': 'connecting', 'hint': 'Waiting for the first WireGuard handshake.'}
    return {'state': 'no_handshake',
            'hint': ("No WireGuard handshake yet — the private host's outbound UDP to the "
                     "edge may be blocked by its network. A relay (roadmap #21) would be "
                     "needed to traverse it.")}
