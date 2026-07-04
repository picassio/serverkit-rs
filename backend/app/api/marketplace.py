"""Marketplace API — the extension registry surface.

The legacy DB-seeded ``Extension``/``ExtensionInstall`` catalog was retired
(#51): nothing ever populated it on a real panel, so it fed Browse an empty
third lane that was redundant with the three real sources — the builtin
folder scan (``/plugins/builtin``), the remote registry (below), and live
``InstalledPlugin`` state. This blueprint now serves only the registry.
"""
from flask import Blueprint, jsonify
from flask_jwt_extended import jwt_required
from app.services.audit_service import AuditService
from app.models.audit_log import AuditLog

marketplace_bp = Blueprint('marketplace', __name__)


def get_current_user():
    from flask_jwt_extended import get_jwt_identity
    from app.models.user import User
    return User.query.get(get_jwt_identity())


@marketplace_bp.route('/registry', methods=['GET'])
@jwt_required()
def list_registry():
    """Return the remote-registry extensions (with live install state), for the
    Browse merge. Read-only; offline-tolerant (falls back to a bundled index)."""
    from app.services import registry_service
    return jsonify({
        'extensions': registry_service.list_catalog(),
        'source': registry_service.registry_source_label(),
    })


@marketplace_bp.route('/registry/<slug>/install', methods=['POST'])
@jwt_required()
def install_registry(slug):
    """Install a registry extension by slug. Checksum-verified (the entry's
    sha256, when present, must match before extraction)."""
    user = get_current_user()
    if not user or not user.is_admin:
        return jsonify({'error': 'Admin access required'}), 403

    from app.services.plugin_service import install_registry_extension
    try:
        plugin = install_registry_extension(slug, user_id=user.id)
        AuditService.log(
            action=AuditLog.ACTION_RESOURCE_CREATE,
            user_id=user.id,
            target_type='plugin',
            target_id=plugin.id,
            details={'name': plugin.name, 'version': plugin.version, 'source': 'registry'},
        )
        return jsonify(plugin.to_dict()), 201
    except ValueError as e:
        return jsonify({'error': str(e)}), 400
    except Exception:
        import logging, uuid
        ref = uuid.uuid4().hex[:8]
        logging.getLogger(__name__).exception('Registry install failed (ref=%s)', ref)
        return jsonify({'error': 'Installation failed. Check server logs.', 'ref': ref}), 500
