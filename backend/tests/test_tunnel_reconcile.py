"""Phase 3 #19: panel-authoritative tunnel reconcile.

Proves reconcile_server re-issues the right agent commands for each side
of a tunnel, by monkeypatching agent_registry.send_command to record calls
(no live agent needed).
"""

from app import db
from app.models.server import Server
from app.models.tunnel import Tunnel
from app.models.exposed_service import ExposedService
from app.services import agent_registry as ar_mod
from app.services.tunnel_broker_service import TunnelBrokerService


def _server(name, ip=None):
    s = Server(name=name, ip_address=ip, status='online')
    db.session.add(s)
    db.session.commit()
    return s


def _record_send(monkeypatch):
    calls = []

    def fake_send(server_id, action, params=None, user_id=None, timeout=30.0):
        calls.append({'server_id': server_id, 'action': action, 'params': params or {}})
        return {'success': True, 'data': {}}

    monkeypatch.setattr(ar_mod.agent_registry, 'send_command', fake_send)
    return calls


def _tunnel(edge, priv, iface, third):
    t = Tunnel(
        edge_server_id=edge.id, private_server_id=priv.id,
        interface_name=iface, subnet='10.88.%d.0/24' % third,
        edge_wg_ip='10.88.%d.1' % third, private_wg_ip='10.88.%d.2' % third,
        listen_port=51820, edge_pubkey='EDGEPUB', private_pubkey='PRIVPUB',
        status='up',
    )
    db.session.add(t)
    db.session.commit()
    return t


def test_reconcile_private_side(app, monkeypatch):
    calls = _record_send(monkeypatch)
    edge = _server('edge', ip='203.0.113.9')
    priv = _server('home')
    t = _tunnel(edge, priv, 'skwgrec01', 7)
    db.session.add(ExposedService(tunnel_id=t.id, hostname='jelly.example.com', port=8096, status='published'))
    db.session.commit()

    res = TunnelBrokerService.reconcile_server(priv.id)
    assert res and res[0]['ok']

    actions = [c['action'] for c in calls]
    assert 'wireguard:interface:up' in actions
    assert 'wireguard:peer:set' in actions
    assert 'wireguard:forward' in actions

    peer = next(c['params'] for c in calls if c['action'] == 'wireguard:peer:set')
    assert peer['public_key'] == 'EDGEPUB'                 # private peers with the edge
    assert peer['endpoint'] == '203.0.113.9:51820'         # dials the edge endpoint
    assert peer['persistent_keepalive'] == 25

    fwd = next(c['params'] for c in calls if c['action'] == 'wireguard:forward')
    assert fwd['listen_port'] == 8096 and fwd['target_port'] == 8096
    assert fwd['listen_ip'] == '10.88.7.2'


def test_reconcile_edge_side(app, monkeypatch):
    calls = _record_send(monkeypatch)
    edge = _server('edge2', ip='198.51.100.4')
    priv = _server('home2')
    _tunnel(edge, priv, 'skwgrec02', 8)

    TunnelBrokerService.reconcile_server(edge.id)

    actions = [c['action'] for c in calls]
    assert 'wireguard:interface:up' in actions
    assert 'wireguard:peer:set' in actions
    assert 'firewall:allow_port' in actions                # edge re-opens its UDP port

    peer = next(c['params'] for c in calls if c['action'] == 'wireguard:peer:set')
    assert peer['public_key'] == 'PRIVPUB'                 # edge peers with the private host
    assert 'endpoint' not in peer                          # edge has no endpoint for the private peer


def test_reconcile_skips_errored_tunnels(app, monkeypatch):
    calls = _record_send(monkeypatch)
    edge = _server('edge3', ip='192.0.2.7')
    priv = _server('home3')
    t = _tunnel(edge, priv, 'skwgrec03', 9)
    t.status = 'error'
    db.session.commit()

    res = TunnelBrokerService.reconcile_server(priv.id)
    assert res == []
    assert calls == []
