"""PR Preview Environments API.

Mounted by the host at ``/api/v1/apps`` so every route here is nested under an
application id, e.g. ``GET /api/v1/apps/<app_id>/previews``.
"""
from flask import Blueprint, request, jsonify
from flask_jwt_extended import jwt_required

from app.middleware.rbac import developer_required
from app.models import Application
from app.models.application_preview import ApplicationPreview
from app.services.preview_service import PreviewService

previews_bp = Blueprint('previews', __name__)


def _get_app_or_404(app_id):
    app = Application.query.get(app_id)
    if not app:
        return None, (jsonify({'error': 'Application not found'}), 404)
    return app, None


@previews_bp.route('/<int:app_id>/previews', methods=['GET'])
@jwt_required()
def list_previews(app_id):
    """List the non-destroyed previews for an application (newest first)."""
    app, err = _get_app_or_404(app_id)
    if err:
        return err
    previews = ApplicationPreview.query.filter(
        ApplicationPreview.application_id == app_id,
        ApplicationPreview.status != ApplicationPreview.STATUS_DESTROYED,
    ).order_by(ApplicationPreview.pr_number.desc()).all()
    return jsonify({'previews': [p.to_dict() for p in previews]}), 200


@previews_bp.route('/<int:app_id>/previews/settings', methods=['GET'])
@jwt_required()
def get_preview_settings(app_id):
    """Return the per-app preview settings (defaults when never configured)."""
    app, err = _get_app_or_404(app_id)
    if err:
        return err
    settings = PreviewService.get_settings(app_id)
    return jsonify(settings.to_dict()), 200


@previews_bp.route('/<int:app_id>/previews/settings', methods=['PUT'])
@developer_required
def update_preview_settings(app_id):
    """Enable/disable previews and set the domain template + TTL for an app."""
    app, err = _get_app_or_404(app_id)
    if err:
        return err
    data = request.get_json(silent=True) or {}
    try:
        result = PreviewService.enable_previews(app_id, data)
    except Exception as exc:  # pragma: no cover - defensive
        return jsonify({'error': str(exc)}), 400
    return jsonify(result), 200


@previews_bp.route('/<int:app_id>/previews/sync', methods=['POST'])
@developer_required
def sync_previews(app_id):
    """Reconcile this app's previews against its open PRs (best-effort)."""
    app, err = _get_app_or_404(app_id)
    if err:
        return err
    result = PreviewService.sync_previews(app_id)
    status = 400 if result.get('error') else 200
    return jsonify(result), status


@previews_bp.route('/<int:app_id>/previews/<int:preview_id>/redeploy', methods=['POST'])
@developer_required
def redeploy_preview(app_id, preview_id):
    """Re-provision a preview for the same PR (idempotent update-in-place)."""
    app, err = _get_app_or_404(app_id)
    if err:
        return err
    preview = ApplicationPreview.query.filter_by(
        id=preview_id, application_id=app_id).first()
    if not preview:
        return jsonify({'error': 'Preview not found'}), 404
    result = PreviewService.create_preview(app, {
        'pr_number': preview.pr_number,
        'branch': preview.branch,
        'pr_title': preview.pr_title,
        'commit_sha': preview.commit_sha,
    })
    status = 400 if result.get('error') else 200
    return jsonify(result), status


@previews_bp.route('/<int:app_id>/previews/<int:preview_id>', methods=['DELETE'])
@developer_required
def destroy_preview(app_id, preview_id):
    """Tear down and mark a preview destroyed."""
    app, err = _get_app_or_404(app_id)
    if err:
        return err
    preview = ApplicationPreview.query.filter_by(
        id=preview_id, application_id=app_id).first()
    if not preview:
        return jsonify({'error': 'Preview not found'}), 404
    result = PreviewService.destroy_preview(preview_id)
    status = 400 if result.get('error') else 200
    return jsonify(result), status
