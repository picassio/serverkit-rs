"""Cloudflare operations API — zone settings (SSL/TLS, Speed, Caching, Security)
and one-click hardening, on top of the existing Cloudflare DNS connection.

Zones are addressed by their ServerKit ``DNSZone`` id (same as the ``/dns`` API);
the service resolves the Cloudflare credential + zone id server-side. Reads are
available to any authenticated user; writes require admin. Every mutating call is
captured by the global audit fallback (method, route args, sanitized body).
"""
from flask import Blueprint, request, jsonify
from flask_jwt_extended import jwt_required, get_jwt_identity

from .cloudflare_service import CloudflareService, CloudflareError

cloudflare_bp = Blueprint('cloudflare', __name__)


def _require_admin():
    """Return the current user if admin, else ``None``."""
    from app.models.user import User
    user = User.query.get(get_jwt_identity())
    return user if (user and user.is_admin) else None


def _service_response(res):
    """Map a service result dict to JSON. A failed *provider* call (reached
    Cloudflare, got an error back) is a 502; resolution errors are raised as
    CloudflareError upstream and handled separately as 400s."""
    if not res.get('success'):
        return jsonify({'error': res.get('error', 'Cloudflare request failed')}), 502
    return jsonify(res)


# ── Zone settings ────────────────────────────────────────────────────────────

@cloudflare_bp.route('/zones/<int:zone_id>/settings', methods=['GET'])
@jwt_required()
def get_zone_settings(zone_id):
    try:
        res = CloudflareService.get_settings(zone_id)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/settings/apply-preset', methods=['POST'])
@jwt_required()
def apply_preset(zone_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    try:
        res = CloudflareService.apply_recommended(zone_id)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    # A preset is best-effort across many toggles; return 200 with the per-setting
    # report even when the plan gated some (the UI surfaces partial success).
    return jsonify(res)


@cloudflare_bp.route('/zones/<int:zone_id>/settings/<setting_id>', methods=['GET'])
@jwt_required()
def get_zone_setting(zone_id, setting_id):
    try:
        res = CloudflareService.get_setting(zone_id, setting_id)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/settings/<setting_id>', methods=['PATCH'])
@jwt_required()
def update_zone_setting(zone_id, setting_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    data = request.get_json(silent=True) or {}
    if 'value' not in data:
        return jsonify({'error': 'A "value" field is required'}), 400
    try:
        res = CloudflareService.update_setting(zone_id, setting_id, data['value'])
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


# ── Cache ────────────────────────────────────────────────────────────────────

@cloudflare_bp.route('/zones/<int:zone_id>/purge-cache', methods=['POST'])
@jwt_required()
def purge_cache(zone_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    data = request.get_json(silent=True) or {}
    try:
        res = CloudflareService.purge_cache(
            zone_id,
            everything=bool(data.get('purge_everything') or data.get('everything')),
            files=data.get('files'), hosts=data.get('hosts'),
            prefixes=data.get('prefixes'), tags=data.get('tags'))
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


# ── WAF custom rules ─────────────────────────────────────────────────────────

@cloudflare_bp.route('/zones/<int:zone_id>/waf/rules', methods=['GET'])
@jwt_required()
def list_waf_rules(zone_id):
    try:
        res = CloudflareService.list_waf_rules(zone_id)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/waf/rules', methods=['POST'])
@jwt_required()
def add_waf_rule(zone_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    data = request.get_json(silent=True) or {}
    try:
        res = CloudflareService.add_waf_rule(
            zone_id,
            description=data.get('description'),
            expression=data.get('expression'),
            action=data.get('action'),
            enabled=data.get('enabled', True))
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/waf/presets/<preset_key>', methods=['POST'])
@jwt_required()
def apply_waf_preset(zone_id, preset_key):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    data = request.get_json(silent=True) or {}
    try:
        res = CloudflareService.apply_waf_preset(zone_id, preset_key, data.get('params') or {})
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/waf/rulesets/<ruleset_id>/rules/<rule_id>',
                     methods=['PATCH'])
@jwt_required()
def update_waf_rule(zone_id, ruleset_id, rule_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    data = request.get_json(silent=True) or {}
    try:
        res = CloudflareService.update_waf_rule(zone_id, ruleset_id, rule_id, data)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/waf/rulesets/<ruleset_id>/rules/<rule_id>',
                     methods=['DELETE'])
@jwt_required()
def delete_waf_rule(zone_id, ruleset_id, rule_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    try:
        res = CloudflareService.delete_waf_rule(zone_id, ruleset_id, rule_id)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


# ── Workers (edge hosting) ───────────────────────────────────────────────────

@cloudflare_bp.route('/zones/<int:zone_id>/workers', methods=['GET'])
@jwt_required()
def list_workers(zone_id):
    try:
        res = CloudflareService.list_workers(zone_id)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/workers', methods=['POST'])
@jwt_required()
def deploy_worker(zone_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    data = request.get_json(silent=True) or {}
    try:
        res = CloudflareService.deploy_worker(
            zone_id,
            name=data.get('name'),
            code=data.get('code'),
            compatibility_date=data.get('compatibility_date'),
            route_pattern=data.get('route_pattern'))
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/workers/routes', methods=['POST'])
@jwt_required()
def add_worker_route(zone_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    data = request.get_json(silent=True) or {}
    try:
        res = CloudflareService.add_worker_route(zone_id, data.get('pattern'), data.get('script'))
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/workers/routes/<route_id>', methods=['DELETE'])
@jwt_required()
def delete_worker_route(zone_id, route_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    try:
        res = CloudflareService.delete_worker_route(zone_id, route_id)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


# Note: this dynamic <name> route is registered last so it can't shadow the more
# specific /workers/routes paths above.
@cloudflare_bp.route('/zones/<int:zone_id>/workers/<name>', methods=['DELETE'])
@jwt_required()
def delete_worker(zone_id, name):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    try:
        res = CloudflareService.delete_worker(zone_id, name)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


# ── Tunnels (cloudflared) ────────────────────────────────────────────────────

@cloudflare_bp.route('/zones/<int:zone_id>/tunnels', methods=['GET'])
@jwt_required()
def list_tunnels(zone_id):
    try:
        res = CloudflareService.list_tunnels(zone_id)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/tunnels', methods=['POST'])
@jwt_required()
def create_tunnel(zone_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    data = request.get_json(silent=True) or {}
    try:
        res = CloudflareService.create_tunnel(zone_id, data.get('name'))
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/tunnels/<tunnel_id>/install', methods=['GET'])
@jwt_required()
def tunnel_install(zone_id, tunnel_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    try:
        res = CloudflareService.get_tunnel_install(zone_id, tunnel_id)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/tunnels/<tunnel_id>/hostnames', methods=['GET'])
@jwt_required()
def tunnel_hostnames(zone_id, tunnel_id):
    try:
        res = CloudflareService.get_tunnel_hostnames(zone_id, tunnel_id)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/tunnels/<tunnel_id>/hostnames', methods=['POST'])
@jwt_required()
def add_tunnel_hostname(zone_id, tunnel_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    data = request.get_json(silent=True) or {}
    try:
        res = CloudflareService.add_tunnel_hostname(
            zone_id, tunnel_id, data.get('hostname'), data.get('service'))
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/tunnels/<tunnel_id>/hostnames', methods=['DELETE'])
@jwt_required()
def remove_tunnel_hostname(zone_id, tunnel_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    data = request.get_json(silent=True) or {}
    try:
        res = CloudflareService.remove_tunnel_hostname(zone_id, tunnel_id, data.get('hostname'))
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/tunnels/<tunnel_id>', methods=['DELETE'])
@jwt_required()
def delete_tunnel(zone_id, tunnel_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    try:
        res = CloudflareService.delete_tunnel(zone_id, tunnel_id)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


# ── Developer platform: R2 / KV / D1 ─────────────────────────────────────────

@cloudflare_bp.route('/zones/<int:zone_id>/storage', methods=['GET'])
@jwt_required()
def list_storage(zone_id):
    try:
        res = CloudflareService.list_storage(zone_id)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/storage/r2', methods=['POST'])
@jwt_required()
def create_r2_bucket(zone_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    data = request.get_json(silent=True) or {}
    try:
        res = CloudflareService.create_r2_bucket(zone_id, data.get('name'))
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/storage/r2/<bucket>', methods=['DELETE'])
@jwt_required()
def delete_r2_bucket(zone_id, bucket):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    try:
        res = CloudflareService.delete_r2_bucket(zone_id, bucket)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/storage/kv', methods=['POST'])
@jwt_required()
def create_kv_namespace(zone_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    data = request.get_json(silent=True) or {}
    try:
        res = CloudflareService.create_kv_namespace(zone_id, data.get('title'))
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/storage/kv/<namespace_id>', methods=['DELETE'])
@jwt_required()
def delete_kv_namespace(zone_id, namespace_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    try:
        res = CloudflareService.delete_kv_namespace(zone_id, namespace_id)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/storage/d1', methods=['POST'])
@jwt_required()
def create_d1_database(zone_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    data = request.get_json(silent=True) or {}
    try:
        res = CloudflareService.create_d1_database(zone_id, data.get('name'))
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)


@cloudflare_bp.route('/zones/<int:zone_id>/storage/d1/<database_id>', methods=['DELETE'])
@jwt_required()
def delete_d1_database(zone_id, database_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    try:
        res = CloudflareService.delete_d1_database(zone_id, database_id)
    except CloudflareError as e:
        return jsonify({'error': str(e)}), 400
    return _service_response(res)
