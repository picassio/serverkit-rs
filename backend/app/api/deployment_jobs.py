"""Deployment job API endpoints."""

from flask import Blueprint, request, jsonify
from flask_jwt_extended import jwt_required

from app.models.deployment_job import DeploymentJob, DeploymentJobLog
from app.services.deployment_job_service import DeploymentJobService

deployment_jobs_bp = Blueprint('deployment_jobs', __name__)


@deployment_jobs_bp.route('', methods=['GET'])
@jwt_required()
def list_deployment_jobs():
    status = request.args.get('status')
    target_server_id = request.args.get('server_id')
    limit = request.args.get('limit', 50, type=int)
    jobs = DeploymentJobService.list_jobs(
        status=status,
        target_server_id=target_server_id,
        limit=min(limit, 200),
    )
    return jsonify({'jobs': jobs}), 200


@deployment_jobs_bp.route('/<job_id>', methods=['GET'])
@jwt_required()
def get_deployment_job(job_id):
    include_logs = request.args.get('logs', 'true').lower() == 'true'
    job = DeploymentJobService.get_job(job_id, include_logs=include_logs)
    if not job:
        return jsonify({'error': 'Deployment job not found'}), 404
    return jsonify({'job': job}), 200


@deployment_jobs_bp.route('/<job_id>/logs', methods=['GET'])
@jwt_required()
def get_deployment_job_logs(job_id):
    job = DeploymentJob.query.get(job_id)
    if not job:
        return jsonify({'error': 'Deployment job not found'}), 404

    after_id = request.args.get('after_id', type=int)
    query = DeploymentJobLog.query.filter_by(job_id=job_id)
    if after_id:
        query = query.filter(DeploymentJobLog.id > after_id)

    logs = query.order_by(DeploymentJobLog.created_at.asc()).all()
    return jsonify({'logs': [log.to_dict() for log in logs]}), 200
