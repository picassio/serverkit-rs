"""Unified connection registry — one normalized, read-only list of every
external account ServerKit is connected to (source, DNS, infra, registrar,
storage, container registries). The individual write paths still live in their
own blueprints; this is the single source of truth for "what's connected".

Container-registry credentials (for private image pulls) are the one write path
that lives *here*, since a registry is just another external account and has no
other natural home — see ``container_registry_service``.
"""

from flask import Blueprint, jsonify, request

from app.middleware.rbac import admin_required, auth_required, get_current_user
from app.services.connection_registry import ConnectionRegistry
from app.services.container_registry_service import ContainerRegistryService
from app.services.workspace_service import WorkspaceService

connections_bp = Blueprint('connections', __name__)


@connections_bp.route('', methods=['GET'])
@connections_bp.route('/', methods=['GET'])
@admin_required
def list_connections():
    """List every connected external account (secret-free). Admin-only — these are
    server-wide credentials (Cloudflare tokens, cloud keys, …), not personal
    settings, so the whole Connections surface lives under Administration."""
    user = get_current_user()
    return jsonify({'connections': ConnectionRegistry.list_all(
        user_id=user.id if user else None)})


# ── Container registries ─────────────────────────────────────────────────────
# CRUD + a login round-trip test. Listing is available to any authenticated user
# (the app-create flow needs it to offer a registry picker); mutations are
# admin-only, matching every other credential store.

def _workspace_id():
    user = get_current_user()
    return WorkspaceService.resolve_workspace_id(
        user, request.headers.get('X-Workspace-Id') or request.args.get('workspace_id'))


@connections_bp.route('/registries', methods=['GET'])
@auth_required()
def list_registries():
    registries = ContainerRegistryService.list_registries(workspace_id=_workspace_id())
    return jsonify({'registries': [r.to_dict() for r in registries]})


@connections_bp.route('/registries', methods=['POST'])
@admin_required
def create_registry():
    data = request.get_json() or {}
    name = (data.get('name') or '').strip()
    if not name:
        return jsonify({'error': 'name is required'}), 400
    user = get_current_user()
    registry = ContainerRegistryService.create(
        name=name,
        provider=data.get('provider') or 'generic',
        registry_url=data.get('registry_url'),
        username=data.get('username'),
        secret=data.get('secret'),
        workspace_id=_workspace_id(),
        created_by=user.id if user else None,
    )
    return jsonify({'registry': registry.to_dict()}), 201


@connections_bp.route('/registries/<int:registry_id>', methods=['PUT'])
@admin_required
def update_registry(registry_id):
    registry = ContainerRegistryService.get(registry_id)
    if not registry:
        return jsonify({'error': 'Registry not found'}), 404
    data = request.get_json() or {}
    registry = ContainerRegistryService.update(
        registry,
        name=data.get('name'),
        provider=data.get('provider'),
        registry_url=data.get('registry_url'),
        username=data.get('username'),
        secret=data.get('secret'),
    )
    return jsonify({'registry': registry.to_dict()})


@connections_bp.route('/registries/<int:registry_id>', methods=['DELETE'])
@admin_required
def delete_registry(registry_id):
    registry = ContainerRegistryService.get(registry_id)
    if not registry:
        return jsonify({'error': 'Registry not found'}), 404
    ContainerRegistryService.delete(registry)
    return jsonify({'success': True})


@connections_bp.route('/registries/<int:registry_id>/test', methods=['POST'])
@admin_required
def test_registry(registry_id):
    registry = ContainerRegistryService.get(registry_id)
    if not registry:
        return jsonify({'error': 'Registry not found'}), 404
    result = ContainerRegistryService.test_connection(registry)
    if result.get('success'):
        return jsonify({'success': True, 'message': f'Logged in to {registry.login_host()}'})
    return jsonify({'success': False, 'error': result.get('error', 'Login failed')}), 400
