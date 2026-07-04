"""Tunnels API (roadmap Phase 1 — #9).

Brokers WireGuard pairings between two agents (a public-IP edge + a NAT'd
private host) so a service behind NAT can be reached over the tunnel.
See docs/REMOTE_ACCESS_ROADMAP.md.
"""

from flask import Blueprint, request, jsonify
from flask_jwt_extended import jwt_required, get_jwt_identity

from app.models.tunnel import Tunnel
from app.services.tunnel_broker_service import TunnelBrokerService, TunnelBrokerError
from app.services.tunnel_publish_service import TunnelPublishService, TunnelPublishError
from app.middleware.rbac import developer_required

tunnels_bp = Blueprint('tunnels', __name__)


def _error(e):
    return jsonify({'error': e.message, 'code': e.code}), e.status


@tunnels_bp.route('/', methods=['GET'])
@jwt_required()
def list_tunnels():
    tunnels = Tunnel.query.order_by(Tunnel.created_at.desc()).all()
    return jsonify({'tunnels': [t.to_dict() for t in tunnels]})


@tunnels_bp.route('/', methods=['POST'])
@jwt_required()
@developer_required
def create_tunnel():
    data = request.get_json(silent=True) or {}
    edge = (data.get('edge_server_id') or '').strip()
    private = (data.get('private_server_id') or '').strip()
    name = (data.get('name') or '').strip() or None
    if not edge or not private:
        return jsonify({'error': 'edge_server_id and private_server_id are required'}), 400
    try:
        tunnel = TunnelBrokerService.create_tunnel(edge, private, name=name, user_id=get_jwt_identity())
    except TunnelBrokerError as e:
        return _error(e)
    body = tunnel.to_dict()
    fw = TunnelBrokerService.firewall_hint(tunnel.listen_port)
    auto = getattr(tunnel, 'firewall_status', None)
    if auto is not None:
        fw['auto_open'] = auto  # outcome of the agent-driven open (#10)
    body['firewall'] = fw
    return jsonify(body), 201


@tunnels_bp.route('/<tunnel_id>', methods=['GET'])
@jwt_required()
def get_tunnel(tunnel_id):
    try:
        tunnel = TunnelBrokerService.get_tunnel(tunnel_id)
    except TunnelBrokerError as e:
        return _error(e)
    # Default: refresh live status. Pass ?refresh=0 for the cached row only.
    if request.args.get('refresh', '1') in ('0', 'false', 'no'):
        return jsonify({'tunnel': tunnel.to_dict()})
    try:
        return jsonify(TunnelBrokerService.refresh_status(tunnel, user_id=get_jwt_identity()))
    except TunnelBrokerError as e:
        return _error(e)


@tunnels_bp.route('/<tunnel_id>', methods=['DELETE'])
@jwt_required()
@developer_required
def delete_tunnel(tunnel_id):
    try:
        TunnelBrokerService.teardown_tunnel(tunnel_id, user_id=get_jwt_identity())
    except TunnelBrokerError as e:
        return _error(e)
    return jsonify({'success': True})


# ---- exposed services (Phase 2: publish a service over a tunnel) --------

@tunnels_bp.route('/<tunnel_id>/services', methods=['GET'])
@jwt_required()
def list_services(tunnel_id):
    services = TunnelPublishService.list_for_tunnel(tunnel_id)
    return jsonify({'services': [s.to_dict() for s in services]})


@tunnels_bp.route('/<tunnel_id>/services', methods=['POST'])
@jwt_required()
@developer_required
def publish_service(tunnel_id):
    data = request.get_json(silent=True) or {}
    hostname = (data.get('hostname') or '').strip()
    port = data.get('port')
    if not hostname or port is None:
        return jsonify({'error': 'hostname and port are required'}), 400
    try:
        svc, steps = TunnelPublishService.publish(
            tunnel_id, hostname, port,
            require_auth=bool(data.get('require_auth')),
            auth_username=(data.get('auth_username') or '').strip() or None,
            auth_password=data.get('auth_password'),
            ssl=data.get('ssl', True),
            email=(data.get('email') or '').strip() or None,
            user_id=get_jwt_identity(),
        )
    except TunnelPublishError as e:
        return _error(e)
    body = svc.to_dict()
    body['steps'] = steps
    return jsonify(body), 201


@tunnels_bp.route('/<tunnel_id>/services/<service_id>', methods=['DELETE'])
@jwt_required()
@developer_required
def unpublish_service(tunnel_id, service_id):
    try:
        TunnelPublishService.unpublish(service_id, user_id=get_jwt_identity())
    except TunnelPublishError as e:
        return _error(e)
    return jsonify({'success': True})
