"""Unit tests for the tunnel broker's pure helpers (roadmap #7 / #11).

These import only app.services.tunnel_netutil (stdlib-only) so they prove
the subnet allocator, interface-name derivation and health classification
without needing a DB or connected agents.
"""

from app.services import tunnel_netutil as tn


def test_pick_subnet_first_free():
    cidr, edge, priv = tn.pick_subnet([])
    assert cidr == '10.88.0.0/24'
    assert edge == '10.88.0.1'
    assert priv == '10.88.0.2'


def test_pick_subnet_skips_used():
    cidr, edge, priv = tn.pick_subnet(['10.88.0.0/24', '10.88.1.0/24'])
    assert cidr == '10.88.2.0/24'
    assert edge == '10.88.2.1'
    assert priv == '10.88.2.2'


def test_pick_subnet_exhausted():
    used = ['10.88.%d.0/24' % i for i in range(256)]
    try:
        tn.pick_subnet(used)
        assert False, "expected RuntimeError on exhausted pool"
    except RuntimeError:
        pass


def test_interface_name_is_stable_and_kernel_valid():
    name = tn.interface_name_for('3fa9c2b1-dead-beef-0000-111122223333')
    assert name == 'skwg3fa9c2b1'
    assert len(name) <= 15
    assert name.replace('skwg', '').isalnum()


def test_derive_status():
    now = 1_000_000
    assert tn.derive_status(now - 10, now) == 'up'
    assert tn.derive_status(now - 10_000, now) == 'degraded'
    assert tn.derive_status(0, now) == 'pending'
    assert tn.derive_status(now, now, interface_up=False) == 'down'


def test_validate_endpoint_host():
    assert tn.validate_endpoint_host('203.0.113.5') is True
    assert tn.validate_endpoint_host('10.0.0.5') is True       # private edge OK
    assert tn.validate_endpoint_host('127.0.0.1') is False     # loopback
    assert tn.validate_endpoint_host('0.0.0.0') is False       # unspecified
    assert tn.validate_endpoint_host('') is False
    assert tn.validate_endpoint_host('not-an-ip') is False


def test_pick_listen_port():
    assert tn.pick_listen_port([]) == 51820
    assert tn.pick_listen_port([51820]) == 51821
    assert tn.pick_listen_port([51820, 51821, 51823]) == 51822  # fills the gap


def test_pick_listen_port_exhausted():
    used = list(range(tn.DEFAULT_LISTEN_PORT, tn.DEFAULT_LISTEN_PORT + tn.LISTEN_PORT_RANGE))
    try:
        tn.pick_listen_port(used)
        assert False, "expected RuntimeError on exhausted port pool"
    except RuntimeError:
        pass


def test_diagnose_reachability():
    assert tn.diagnose_reachability(0, None, False)['state'] == 'interface_down'
    assert tn.diagnose_reachability(1_700_000_000, 999, True)['state'] == 'ok'
    assert tn.diagnose_reachability(0, 5, True)['state'] == 'connecting'
    d = tn.diagnose_reachability(0, 600, True)
    assert d['state'] == 'no_handshake'
    assert 'blocked' in d['hint'].lower()
