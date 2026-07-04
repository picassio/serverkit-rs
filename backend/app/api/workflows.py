import json
import secrets
from datetime import datetime
from flask import Blueprint, request, jsonify
from flask_jwt_extended import jwt_required, get_jwt_identity
from app import db
from app.models import Workflow, User, WorkflowExecution, WorkflowLog

workflows_bp = Blueprint('workflows', __name__)


def _validate_workflow_graph(data):
    """Validate workflow graph for cycles if nodes/edges are present."""
    nodes = data.get('nodes', [])
    edges = data.get('edges', [])
    if nodes and edges:
        from app.services.workflow_engine import WorkflowEngine
        err = WorkflowEngine.validate_graph(nodes, edges)
        if err:
            return err
    return None


@workflows_bp.route('', methods=['GET'])
@jwt_required()
def get_workflows():
    """Get all workflows for the current user."""
    current_user_id = get_jwt_identity()
    user = User.query.get(current_user_id)

    if user.role == 'admin':
        workflows = Workflow.query.all()
    else:
        workflows = Workflow.query.filter_by(user_id=current_user_id).all()

    return jsonify({
        'workflows': [w.to_dict() for w in workflows]
    })


@workflows_bp.route('/<int:workflow_id>', methods=['GET'])
@jwt_required()
def get_workflow(workflow_id):
    """Get a single workflow by ID."""
    current_user_id = get_jwt_identity()
    user = User.query.get(current_user_id)
    workflow = Workflow.query.get(workflow_id)

    if not workflow:
        return jsonify({'error': 'Workflow not found'}), 404

    if user.role != 'admin' and workflow.user_id != current_user_id:
        return jsonify({'error': 'Access denied'}), 403

    return jsonify(workflow.to_dict())


@workflows_bp.route('', methods=['POST'])
@jwt_required()
def create_workflow():
    """Create a new workflow."""
    current_user_id = get_jwt_identity()
    data = request.get_json()

    if not data:
        return jsonify({'error': 'No data provided'}), 400

    name = data.get('name')
    if not name:
        return jsonify({'error': 'Name is required'}), 400

    # Validate graph for cycles
    cycle_err = _validate_workflow_graph(data)
    if cycle_err:
        return jsonify({'error': cycle_err}), 400

    # Auto-generate webhook_id if trigger is webhook
    trigger_config = data.get('trigger_config', {})
    if data.get('trigger_type') == 'webhook' and not trigger_config.get('webhook_id'):
        trigger_config['webhook_id'] = secrets.token_urlsafe(24)

    workflow = Workflow(
        name=name,
        description=data.get('description', ''),
        nodes=json.dumps(data.get('nodes', [])),
        edges=json.dumps(data.get('edges', [])),
        viewport=json.dumps(data.get('viewport')) if data.get('viewport') else None,
        is_active=data.get('is_active', False),
        trigger_type=data.get('trigger_type', 'manual'),
        trigger_config=json.dumps(trigger_config),
        user_id=current_user_id
    )

    db.session.add(workflow)
    db.session.commit()

    return jsonify({
        'message': 'Workflow created successfully',
        'workflow': workflow.to_dict()
    }), 201


@workflows_bp.route('/<int:workflow_id>', methods=['PUT'])
@jwt_required()
def update_workflow(workflow_id):
    """Update an existing workflow."""
    current_user_id = get_jwt_identity()
    user = User.query.get(current_user_id)
    workflow = Workflow.query.get(workflow_id)

    if not workflow:
        return jsonify({'error': 'Workflow not found'}), 404

    if user.role != 'admin' and workflow.user_id != current_user_id:
        return jsonify({'error': 'Access denied'}), 403

    data = request.get_json()
    if not data:
        return jsonify({'error': 'No data provided'}), 400

    # Validate graph for cycles
    cycle_err = _validate_workflow_graph(data)
    if cycle_err:
        return jsonify({'error': cycle_err}), 400

    if 'name' in data:
        workflow.name = data['name']
    if 'description' in data:
        workflow.description = data['description']
    if 'nodes' in data:
        workflow.nodes = json.dumps(data['nodes'])
    if 'edges' in data:
        workflow.edges = json.dumps(data['edges'])
    if 'viewport' in data:
        workflow.viewport = json.dumps(data['viewport']) if data['viewport'] else None

    # Automation fields
    if 'is_active' in data:
        workflow.is_active = data['is_active']
    if 'trigger_type' in data:
        workflow.trigger_type = data['trigger_type']
    if 'trigger_config' in data:
        trigger_config = data['trigger_config']
        # Auto-generate webhook_id if switching to webhook trigger
        if data.get('trigger_type') == 'webhook' and not trigger_config.get('webhook_id'):
            existing = json.loads(workflow.trigger_config) if workflow.trigger_config else {}
            trigger_config['webhook_id'] = existing.get('webhook_id') or secrets.token_urlsafe(24)
        workflow.trigger_config = json.dumps(trigger_config)

    db.session.commit()

    return jsonify({
        'message': 'Workflow updated successfully',
        'workflow': workflow.to_dict()
    })


@workflows_bp.route('/<int:workflow_id>', methods=['DELETE'])
@jwt_required()
def delete_workflow(workflow_id):
    """Delete a workflow."""
    current_user_id = get_jwt_identity()
    user = User.query.get(current_user_id)
    workflow = Workflow.query.get(workflow_id)

    if not workflow:
        return jsonify({'error': 'Workflow not found'}), 404

    if user.role != 'admin' and workflow.user_id != current_user_id:
        return jsonify({'error': 'Access denied'}), 403

    db.session.delete(workflow)
    db.session.commit()

    return jsonify({'message': 'Workflow deleted successfully'})


@workflows_bp.route('/<int:workflow_id>/execute', methods=['POST'])
@jwt_required()
def execute_workflow(workflow_id):
    """Trigger a workflow execution manually."""
    from app.services.workflow_engine import WorkflowEngine

    current_user_id = get_jwt_identity()
    user = User.query.get(current_user_id)
    workflow = Workflow.query.get(workflow_id)

    if not workflow:
        return jsonify({'error': 'Workflow not found'}), 404

    if user.role != 'admin' and workflow.user_id != current_user_id:
        return jsonify({'error': 'Access denied'}), 403

    data = request.get_json() or {}
    context = data.get('context', {})

    execution_id = WorkflowEngine.enqueue_execution(
        workflow_id=workflow_id,
        trigger_type='manual',
        context=context
    )

    return jsonify({
        'message': 'Workflow execution started',
        'execution_id': execution_id
    })


@workflows_bp.route('/<int:workflow_id>/executions', methods=['GET'])
@jwt_required()
def get_workflow_executions(workflow_id):
    """Get execution history for a workflow."""
    current_user_id = get_jwt_identity()
    user = User.query.get(current_user_id)
    workflow = Workflow.query.get(workflow_id)

    if not workflow:
        return jsonify({'error': 'Workflow not found'}), 404

    if user.role != 'admin' and workflow.user_id != current_user_id:
        return jsonify({'error': 'Access denied'}), 403

    executions = WorkflowExecution.query.filter_by(workflow_id=workflow_id).order_by(WorkflowExecution.started_at.desc()).all()

    return jsonify({
        'executions': [e.to_dict() for e in executions]
    })


@workflows_bp.route('/executions/<int:execution_id>', methods=['GET'])
@jwt_required()
def get_execution_details(execution_id):
    """Get details for a specific execution."""
    current_user_id = get_jwt_identity()
    user = User.query.get(current_user_id)
    execution = WorkflowExecution.query.get(execution_id)

    if not execution:
        return jsonify({'error': 'Execution not found'}), 404

    workflow = execution.workflow
    if user.role != 'admin' and workflow.user_id != current_user_id:
        return jsonify({'error': 'Access denied'}), 403

    return jsonify(execution.to_dict())


@workflows_bp.route('/executions/<int:execution_id>/logs', methods=['GET'])
@jwt_required()
def get_execution_logs(execution_id):
    """Get logs for a specific execution."""
    current_user_id = get_jwt_identity()
    user = User.query.get(current_user_id)
    execution = WorkflowExecution.query.get(execution_id)

    if not execution:
        return jsonify({'error': 'Execution not found'}), 404

    workflow = execution.workflow
    if user.role != 'admin' and workflow.user_id != current_user_id:
        return jsonify({'error': 'Access denied'}), 403

    logs = WorkflowLog.query.filter_by(execution_id=execution_id).order_by(WorkflowLog.timestamp.asc()).all()

    return jsonify({
        'logs': [l.to_dict() for l in logs]
    })


@workflows_bp.route('/<int:workflow_id>/deploy', methods=['POST'])
@jwt_required()
def deploy_workflow(workflow_id):
    """
    Deploy all resources from a workflow.
    """
    from app.services.workflow_service import WorkflowService

    current_user_id = get_jwt_identity()

    result = WorkflowService.deploy_workflow(workflow_id, current_user_id)

    if result.get('success'):
        return jsonify(result), 200
    elif result.get('error') == 'Workflow not found':
        return jsonify(result), 404
    elif result.get('error') == 'Access denied':
        return jsonify(result), 403
    else:
        # Partial success or errors - return 200 with error details
        return jsonify(result), 200


@workflows_bp.route('/hooks/<webhook_id>', methods=['POST'])
def webhook_trigger(webhook_id):
    """
    Public webhook endpoint to trigger a workflow.
    No JWT required — the webhook_id acts as the authentication token.
    """
    from app.services.workflow_engine import WorkflowEngine, CycleDetectedError

    # Find the workflow with this webhook_id
    workflows = Workflow.query.filter_by(
        is_active=True,
        trigger_type='webhook'
    ).all()

    target_workflow = None
    for wf in workflows:
        try:
            config = json.loads(wf.trigger_config) if wf.trigger_config else {}
            if config.get('webhook_id') == webhook_id:
                target_workflow = wf
                break
        except (json.JSONDecodeError, TypeError):
            continue

    if not target_workflow:
        return jsonify({'error': 'Webhook not found'}), 404

    # Build context from the incoming request
    context = {
        'webhook_id': webhook_id,
        'method': request.method,
        'headers': dict(request.headers),
        'triggered_at': datetime.utcnow().isoformat()
    }

    # Include request body
    if request.is_json:
        context['body'] = request.get_json(silent=True) or {}
    else:
        body = request.get_data(as_text=True)
        context['body'] = body[:8192] if body else ''

    # Include query parameters
    if request.args:
        context['query'] = dict(request.args)

    try:
        execution_id = WorkflowEngine.enqueue_execution(
            workflow_id=target_workflow.id,
            trigger_type='webhook',
            context=context
        )
        return jsonify({
            'message': 'Workflow triggered',
            'execution_id': execution_id,
            'workflow': target_workflow.name
        }), 200
    except CycleDetectedError as e:
        return jsonify({'error': str(e)}), 400
    except Exception as e:
        return jsonify({'error': f'Execution failed: {str(e)}'}), 500


@workflows_bp.route('/validate', methods=['POST'])
@jwt_required()
def validate_workflow():
    """Validate a workflow graph for cycles and other issues."""
    from app.services.workflow_engine import WorkflowEngine

    data = request.get_json()
    if not data:
        return jsonify({'error': 'No data provided'}), 400

    nodes = data.get('nodes', [])
    edges = data.get('edges', [])

    err = WorkflowEngine.validate_graph(nodes, edges)
    if err:
        return jsonify({'valid': False, 'error': err}), 200

    return jsonify({'valid': True}), 200
