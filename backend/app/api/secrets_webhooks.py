"""API endpoints for secrets manager and inbound webhook gateway."""
from datetime import datetime
from functools import wraps

from flask import Blueprint, request
from flask_jwt_extended import get_jwt_identity, jwt_required

from app.services.secret_vault_service import SecretService, SecretVaultService
from app.services.webhook_gateway_service import WebhookGatewayService
from app.services.workspace_service import WorkspaceService
from app.models import User, Workspace

bp = Blueprint('secrets_webhooks', __name__)


def _current_user_id() -> int:
    identity = get_jwt_identity()
    return int(identity) if identity else None


def _resolve_ws_value():
    """Return the raw workspace context from header or query string."""
    return request.headers.get('X-Workspace-Id') or request.args.get('workspace_id')


def _resolve_ws():
    """Resolve the active workspace context (X-Workspace-Id header or
    ?workspace_id) to a workspace id, or None for an unscoped view — mirrors the
    servers/apps scoping (#33). Lenient: a stale/unknown context degrades to
    None rather than erroring."""
    from app.models import User
    from app.services.workspace_service import WorkspaceService
    user = User.query.get(get_jwt_identity())
    return WorkspaceService.resolve_workspace_id(user, _resolve_ws_value())


def _json_or_form() -> dict:
    """Return JSON body if available, otherwise an empty dict."""
    if request.is_json:
        return request.get_json(silent=True) or {}
    return {}


def admin_required(fn):
    """Require an admin user (placeholder for role check)."""
    @wraps(fn)
    @jwt_required()
    def wrapper(*args, **kwargs):
        # Role checks can be added here when the auth system exposes roles.
        return fn(*args, **kwargs)
    return wrapper


# ---------- Secret Vaults ----------

@bp.route('/vaults', methods=['GET'])
@jwt_required()
def list_vaults():
    return {'success': True, 'vaults': SecretVaultService.list_vaults(workspace_id=_resolve_ws())}


@bp.route('/vaults', methods=['POST'])
@jwt_required()
def create_vault():
    data = _json_or_form()
    name = data.get('name')
    if not name:
        return {'error': 'name is required'}, 400
    user = User.query.get(get_jwt_identity())
    # Explicit workspace_id in the body takes precedence over the active context,
    # but the caller must be a member of that workspace.
    explicit_ws = data.get('workspace_id')
    if explicit_ws is not None:
        try:
            explicit_ws = int(explicit_ws)
        except (ValueError, TypeError):
            return {'error': 'workspace_id must be an integer'}, 400
        if Workspace.query.get(explicit_ws) is None:
            return {'error': 'Workspace not found'}, 404
        if not user.is_admin and WorkspaceService.get_user_role(explicit_ws, user.id) is None:
            return {'error': 'Workspace access denied'}, 403
        workspace_id = explicit_ws
    else:
        workspace_id = WorkspaceService.resolve_workspace_id(user, _resolve_ws_value())
    result = SecretVaultService.create_vault(name, description=data.get('description'),
                                             user_id=user.id, workspace_id=workspace_id)
    if not result.get('success'):
        return {'error': result.get('error')}, 409
    return result, 201


@bp.route('/vaults/<int:vault_id>', methods=['GET'])
@jwt_required()
def get_vault(vault_id):
    vault = SecretVaultService.get_vault(vault_id)
    if not vault:
        return {'error': 'Vault not found'}, 404
    return {'success': True, 'vault': vault.to_dict()}


@bp.route('/vaults/<int:vault_id>', methods=['PATCH'])
@jwt_required()
def update_vault(vault_id):
    data = _json_or_form()
    result = SecretVaultService.update_vault(vault_id, name=data.get('name'), description=data.get('description'))
    if not result.get('success'):
        return {'error': result.get('error')}, 404 if 'not found' in result.get('error', '').lower() else 409
    return result


@bp.route('/vaults/<int:vault_id>', methods=['DELETE'])
@jwt_required()
def delete_vault(vault_id):
    result = SecretVaultService.delete_vault(vault_id)
    if not result.get('success'):
        return {'error': result.get('error')}, 404
    return result


# ---------- Secrets ----------

@bp.route('/vaults/<int:vault_id>/secrets', methods=['GET'])
@jwt_required()
def list_secrets(vault_id):
    return {'success': True, 'secrets': SecretService.list_secrets(vault_id)}


@bp.route('/vaults/<int:vault_id>/secrets', methods=['POST'])
@jwt_required()
def create_secret(vault_id):
    data = _json_or_form()
    name = data.get('name')
    value = data.get('value')
    if name is None or value is None:
        return {'error': 'name and value are required'}, 400
    expires_at = None
    if data.get('expires_at'):
        try:
            expires_at = datetime.fromisoformat(data['expires_at'])
        except ValueError:
            return {'error': 'Invalid expires_at format'}, 400
    result = SecretService.create_secret(vault_id, name, value, description=data.get('description'), expires_at=expires_at)
    if not result.get('success'):
        return {'error': result.get('error')}, 409 if 'already exists' in result.get('error', '').lower() else 400
    return result, 201


@bp.route('/vaults/<int:vault_id>/secrets/bulk', methods=['POST'])
@jwt_required()
def bulk_create_secrets(vault_id):
    data = _json_or_form()
    secrets_list = data.get('secrets', [])
    if not isinstance(secrets_list, list):
        return {'error': 'secrets must be a list'}, 400
    result = SecretService.bulk_create_or_update(vault_id, secrets_list)
    return result, 207


@bp.route('/secrets/<int:secret_id>', methods=['GET'])
@jwt_required()
def get_secret(secret_id):
    secret = SecretService.get_secret(secret_id)
    if not secret:
        return {'error': 'Secret not found'}, 404
    return {'success': True, 'secret': secret.to_dict(mask=True)}


@bp.route('/secrets/<int:secret_id>', methods=['PATCH'])
@jwt_required()
def update_secret(secret_id):
    data = _json_or_form()
    expires_at = None
    if data.get('expires_at'):
        try:
            expires_at = datetime.fromisoformat(data['expires_at'])
        except ValueError:
            return {'error': 'Invalid expires_at format'}, 400
    result = SecretService.update_secret(
        secret_id,
        value=data.get('value'),
        description=data.get('description'),
        expires_at=expires_at,
        rotate=data.get('rotate', False),
    )
    if not result.get('success'):
        return {'error': result.get('error')}, 404
    return result


@bp.route('/secrets/<int:secret_id>/reveal', methods=['POST'])
@jwt_required()
def reveal_secret(secret_id):
    result = SecretService.reveal_secret(secret_id)
    if not result.get('success'):
        return {'error': result.get('error')}, 404
    return result


@bp.route('/secrets/<int:secret_id>', methods=['DELETE'])
@jwt_required()
def delete_secret(secret_id):
    result = SecretService.delete_secret(secret_id)
    if not result.get('success'):
        return {'error': result.get('error')}, 404
    return result


# ---------- Webhook Endpoints ----------

@bp.route('/webhooks/endpoints', methods=['GET'])
@jwt_required()
def list_webhook_endpoints():
    return {'success': True, 'endpoints': WebhookGatewayService.list_endpoints(workspace_id=_resolve_ws())}


@bp.route('/webhooks/endpoints', methods=['POST'])
@jwt_required()
def create_webhook_endpoint():
    data = _json_or_form()
    name = data.get('name')
    if not name:
        return {'error': 'name is required'}, 400
    result = WebhookGatewayService.create_endpoint(
        name=name,
        secret=data.get('secret'),
        forward_url=data.get('forward_url'),
        filter_paths=data.get('filter_paths'),
        retry_count=data.get('retry_count', 3),
        user_id=_current_user_id(),
        workspace_id=_resolve_ws(),
    )
    if not result.get('success'):
        return {'error': result.get('error')}, 409
    return result, 201


@bp.route('/webhooks/endpoints/<int:endpoint_id>', methods=['GET'])
@jwt_required()
def get_webhook_endpoint(endpoint_id):
    endpoint = WebhookGatewayService.get_endpoint(endpoint_id)
    if not endpoint:
        return {'error': 'Endpoint not found'}, 404
    return {'success': True, 'endpoint': endpoint.to_dict()}


@bp.route('/webhooks/endpoints/<int:endpoint_id>', methods=['PATCH'])
@jwt_required()
def update_webhook_endpoint(endpoint_id):
    data = _json_or_form()
    result = WebhookGatewayService.update_endpoint(
        endpoint_id,
        name=data.get('name'),
        forward_url=data.get('forward_url'),
        filter_paths=data.get('filter_paths'),
        retry_count=data.get('retry_count'),
        is_active=data.get('is_active'),
    )
    if not result.get('success'):
        return {'error': result.get('error')}, 404 if 'not found' in result.get('error', '').lower() else 409
    return result


@bp.route('/webhooks/endpoints/<int:endpoint_id>/regenerate-secret', methods=['POST'])
@jwt_required()
def regenerate_webhook_secret(endpoint_id):
    result = WebhookGatewayService.regenerate_secret(endpoint_id)
    if not result.get('success'):
        return {'error': result.get('error')}, 404
    return result


@bp.route('/webhooks/endpoints/<int:endpoint_id>', methods=['DELETE'])
@jwt_required()
def delete_webhook_endpoint(endpoint_id):
    result = WebhookGatewayService.delete_endpoint(endpoint_id)
    if not result.get('success'):
        return {'error': result.get('error')}, 404
    return result


@bp.route('/webhooks/endpoints/<int:endpoint_id>/deliveries', methods=['GET'])
@jwt_required()
def list_webhook_deliveries(endpoint_id):
    limit = request.args.get('limit', 50, type=int)
    status = request.args.get('status')
    return {'success': True, 'deliveries': WebhookGatewayService.list_deliveries(endpoint_id, limit=limit, status=status)}


@bp.route('/webhooks/deliveries/<int:delivery_id>/replay', methods=['POST'])
@jwt_required()
def replay_webhook_delivery(delivery_id):
    result = WebhookGatewayService.replay_delivery(delivery_id)
    if not result.get('success'):
        return {'error': result.get('error')}, 404
    return result


# ---------- Public webhook receiver ----------

@bp.route('/webhooks/receive/<string:slug>', methods=['POST'])
def receive_webhook(slug):
    payload = request.get_data() or b''
    headers = dict(request.headers)
    result, status = WebhookGatewayService.receive(slug, payload, headers)
    return result, status
