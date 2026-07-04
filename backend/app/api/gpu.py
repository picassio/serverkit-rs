from flask import Blueprint, jsonify
from flask_jwt_extended import jwt_required
from app.services.gpu_service import GpuService

gpu_bp = Blueprint('gpu', __name__)


@gpu_bp.route('/', methods=['GET'])
@jwt_required()
def gpu_info():
    """Per-GPU stats (utilization, VRAM, temperature, power, fan, driver) plus
    the GPU compute processes, with best-effort container resolution."""
    return jsonify(GpuService.info())
