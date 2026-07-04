"""Read/ops API for the unified job system — the single pane to observe all
background work. Admin-gated, mirroring app/api/queue_bus.py conventions."""
from flask import Blueprint, request, jsonify
from flask_jwt_extended import jwt_required, get_jwt_identity

from app.jobs import registry
from app.jobs.service import JobService, ScheduledJobService

jobs_bp = Blueprint('jobs', __name__)


def _require_admin():
    from app.models.user import User
    user = User.query.get(get_jwt_identity())
    if not user or not user.is_admin:
        return None
    return user


# --- Static routes first; Werkzeug ranks these above the dynamic /<job_id>. ---
@jobs_bp.route('', methods=['GET'])
@jobs_bp.route('/', methods=['GET'])
@jwt_required()
def list_jobs():
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    try:
        limit = min(int(request.args.get('limit', 50)), 200)
        offset = int(request.args.get('offset', 0))
    except (TypeError, ValueError):
        limit, offset = 50, 0
    jobs = JobService.list(
        status=request.args.get('status'),
        kind=request.args.get('kind'),
        owner_type=request.args.get('owner_type'),
        owner_id=request.args.get('owner_id'),
        limit=limit,
        offset=offset,
    )
    return jsonify({'jobs': [j.to_dict() for j in jobs]})


@jobs_bp.route('/stats', methods=['GET'])
@jwt_required()
def job_stats():
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    return jsonify(JobService.stats())


@jobs_bp.route('/kinds', methods=['GET'])
@jwt_required()
def job_kinds():
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    return jsonify({'kinds': registry.registered_kinds()})


@jobs_bp.route('/scheduled', methods=['GET'])
@jwt_required()
def list_scheduled():
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    return jsonify({'scheduled': [s.to_dict() for s in ScheduledJobService.list()]})


@jobs_bp.route('/scheduled/<int:scheduled_id>/run', methods=['POST'])
@jwt_required()
def run_scheduled(scheduled_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    job = ScheduledJobService.run_now(scheduled_id)
    if not job:
        return jsonify({'error': 'Scheduled job not found'}), 404
    return jsonify({'job': job.to_dict()})


@jobs_bp.route('/scheduled/<int:scheduled_id>/enabled', methods=['POST'])
@jwt_required()
def toggle_scheduled(scheduled_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    body = request.get_json(silent=True) or {}
    scheduled = ScheduledJobService.set_enabled(scheduled_id, bool(body.get('enabled', True)))
    if not scheduled:
        return jsonify({'error': 'Scheduled job not found'}), 404
    return jsonify({'scheduled': scheduled.to_dict()})


@jobs_bp.route('/<job_id>', methods=['GET'])
@jwt_required()
def get_job(job_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    job = JobService.get(job_id)
    if not job:
        return jsonify({'error': 'Job not found'}), 404
    return jsonify({'job': job.to_dict(include_payload=True)})


@jobs_bp.route('/<job_id>/cancel', methods=['POST'])
@jwt_required()
def cancel_job(job_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    job = JobService.cancel(job_id)
    if not job:
        return jsonify({'error': 'Job not found'}), 404
    return jsonify({'job': job.to_dict()})


@jobs_bp.route('/<job_id>/retry', methods=['POST'])
@jwt_required()
def retry_job(job_id):
    if not _require_admin():
        return jsonify({'error': 'Admin access required'}), 403
    job = JobService.retry(job_id)
    if not job:
        return jsonify({'error': 'Job not found'}), 404
    return jsonify({'job': job.to_dict()})
