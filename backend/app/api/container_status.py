"""REST surface for the centralized container status aggregator.

Mounted at /api/v1/status (registered in app/__init__.py).
"""

from flask import Blueprint, jsonify
from flask_jwt_extended import jwt_required

from app.services import container_status_service as css

container_status_bp = Blueprint('container_status', __name__)


@container_status_bp.route('/app/<int:app_id>', methods=['GET'])
@jwt_required()
def app_status(app_id):
    """Aggregated container status for a single application."""
    return jsonify(css.get_app_status(app_id))


@container_status_bp.route('/apps', methods=['GET'])
@jwt_required()
def apps_status():
    """Lightweight aggregated status for every application.

    Returns {'statuses': [{app_id, status, total, healthy}, ...]}.
    """
    return jsonify({'statuses': css.list_app_statuses()})
