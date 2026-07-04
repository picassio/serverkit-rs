"""Polymorphic shared resources API — tags + shared variable groups.

Mounted at ``/api/v1/shared``. Everything here is a JWT-protected facade over
:class:`~app.services.shared_resource_service.SharedResourceService`. Secret
variable values are always masked in responses via ``to_dict(mask_secrets=True)``
plus a defense-in-depth pass through :func:`mask_sensitive`.
"""
from flask import Blueprint, jsonify, request
from flask_jwt_extended import jwt_required

from app.services.shared_resource_service import SharedResourceService
from app.models.shared_resource import SharedVariableGroup
from app.utils.sensitive_data_filter import mask_sensitive

shared_resources_bp = Blueprint('shared_resources', __name__)


def _bad(msg, code=400):
    return jsonify({'error': msg}), code


def _require(data, *keys):
    """Return the first missing key name, or None if all present/truthy."""
    for k in keys:
        if data.get(k) in (None, ''):
            return k
    return None


# ---------------------------------------------------------------- metadata

@shared_resources_bp.route('/resource-types', methods=['GET'])
@jwt_required()
def resource_types():
    """The catalog of supported polymorphic resource types."""
    return jsonify({'resource_types': list(SharedResourceService.RESOURCE_TYPES)})


# -------------------------------------------------------------------- tags

@shared_resources_bp.route('/tags', methods=['GET'])
@jwt_required()
def list_tags():
    """List tags for a resource, or resources for a tag.

    ``?resource_type=&resource_id=`` → tags on that resource.
    ``?tag=`` (optionally with ``resource_type``) → resources carrying the tag.
    """
    tag = request.args.get('tag')
    resource_type = request.args.get('resource_type')
    resource_id = request.args.get('resource_id')

    if tag:
        rows = SharedResourceService.list_resources_by_tag(tag, resource_type)
        return jsonify({'resources': [r.to_dict() for r in rows]})

    if not resource_type or resource_id in (None, ''):
        return _bad('resource_type and resource_id (or tag) are required')

    rows = SharedResourceService.list_tags(resource_type, resource_id)
    return jsonify({'tags': [r.to_dict() for r in rows]})


@shared_resources_bp.route('/tags', methods=['POST'])
@jwt_required()
def add_tag():
    data = request.get_json() or {}
    missing = _require(data, 'resource_type', 'resource_id', 'tag')
    if missing:
        return _bad(f'{missing} is required')
    try:
        row = SharedResourceService.add_tag(
            data['resource_type'], data['resource_id'], data['tag']
        )
    except ValueError as e:
        return _bad(str(e))
    return jsonify(row.to_dict()), 201


@shared_resources_bp.route('/tags', methods=['DELETE'])
@jwt_required()
def remove_tag():
    # Accept either body or query params for convenience.
    data = request.get_json(silent=True) or {}
    resource_type = data.get('resource_type') or request.args.get('resource_type')
    resource_id = data.get('resource_id')
    if resource_id is None:
        resource_id = request.args.get('resource_id')
    tag = data.get('tag') or request.args.get('tag')

    if not resource_type or resource_id in (None, '') or not tag:
        return _bad('resource_type, resource_id and tag are required')

    removed = SharedResourceService.remove_tag(resource_type, resource_id, tag)
    return jsonify({'removed': removed})


# --------------------------------------------------------- variable groups

@shared_resources_bp.route('/variable-groups', methods=['GET'])
@jwt_required()
def list_groups():
    groups = SharedResourceService.list_groups(
        scope_type=request.args.get('scope_type'),
        scope_id=request.args.get('scope_id'),
    )
    return jsonify({'groups': [g.to_dict() for g in groups]})


@shared_resources_bp.route('/variable-groups', methods=['POST'])
@jwt_required()
def create_group():
    data = request.get_json() or {}
    missing = _require(data, 'scope_type', 'scope_id', 'name')
    if missing:
        return _bad(f'{missing} is required')
    try:
        group = SharedResourceService.create_group(
            data['scope_type'], data['scope_id'], data['name'],
            data.get('description'),
        )
    except ValueError as e:
        return _bad(str(e))
    return jsonify(group.to_dict(include_variables=True)), 201


@shared_resources_bp.route('/variable-groups/<int:group_id>', methods=['GET'])
@jwt_required()
def get_group(group_id):
    group = SharedResourceService.get_group(group_id)
    if not group:
        return _bad('group not found', 404)
    payload = group.to_dict(include_variables=True, mask_secrets=True)
    payload['attachments'] = [a.to_dict() for a in group.attachments]
    return jsonify(mask_sensitive(payload))


@shared_resources_bp.route('/variable-groups/<int:group_id>', methods=['PUT'])
@jwt_required()
def update_group(group_id):
    data = request.get_json() or {}
    try:
        group = SharedResourceService.update_group(
            group_id, name=data.get('name'), description=data.get('description')
        )
    except ValueError as e:
        return _bad(str(e))
    if not group:
        return _bad('group not found', 404)
    return jsonify(group.to_dict(include_variables=True))


@shared_resources_bp.route('/variable-groups/<int:group_id>', methods=['DELETE'])
@jwt_required()
def delete_group(group_id):
    if not SharedResourceService.delete_group(group_id):
        return _bad('group not found', 404)
    return jsonify({'message': 'group deleted'})


# --------------------------------------------- variables within a group

@shared_resources_bp.route('/variable-groups/<int:group_id>/variables', methods=['POST'])
@jwt_required()
def add_variable(group_id):
    data = request.get_json() or {}
    if data.get('key') in (None, ''):
        return _bad('key is required')
    try:
        var = SharedResourceService.set_variable(
            group_id, data['key'], data.get('value', ''),
            is_secret=bool(data.get('is_secret', False)),
            target_service=data.get('target_service') if 'target_service' in data else None,
        )
    except ValueError as e:
        return _bad(str(e))
    if not var:
        return _bad('group not found', 404)
    return jsonify(var.to_dict(mask_secrets=True)), 201


@shared_resources_bp.route(
    '/variable-groups/<int:group_id>/variables/<int:variable_id>', methods=['PUT'])
@jwt_required()
def update_variable(group_id, variable_id):
    data = request.get_json() or {}
    _kwargs = {'value': data.get('value'), 'is_secret': data.get('is_secret')}
    if 'target_service' in data:
        # Only forward when the client sent it, so it's preserved otherwise.
        _kwargs['target_service'] = data.get('target_service')
    var = SharedResourceService.update_variable(variable_id, **_kwargs)
    if not var or var.group_id != group_id:
        return _bad('variable not found', 404)
    return jsonify(var.to_dict(mask_secrets=True))


@shared_resources_bp.route(
    '/variable-groups/<int:group_id>/variables/<int:variable_id>', methods=['DELETE'])
@jwt_required()
def delete_variable(group_id, variable_id):
    from app.models.shared_resource import SharedVariable
    var = SharedVariable.query.get(variable_id)
    if not var or var.group_id != group_id:
        return _bad('variable not found', 404)
    SharedResourceService.delete_variable(variable_id)
    return jsonify({'message': 'variable deleted'})


# --------------------------------------------------------------- attach

@shared_resources_bp.route('/variable-groups/<int:group_id>/attach', methods=['POST'])
@jwt_required()
def attach_group(group_id):
    data = request.get_json() or {}
    missing = _require(data, 'resource_type', 'resource_id')
    if missing:
        return _bad(f'{missing} is required')
    att = SharedResourceService.attach_group(
        group_id, data['resource_type'], data['resource_id']
    )
    if not att:
        return _bad('group not found', 404)
    return jsonify(att.to_dict()), 201


@shared_resources_bp.route('/variable-groups/<int:group_id>/detach', methods=['POST'])
@jwt_required()
def detach_group(group_id):
    data = request.get_json() or {}
    missing = _require(data, 'resource_type', 'resource_id')
    if missing:
        return _bad(f'{missing} is required')
    removed = SharedResourceService.detach_group(
        group_id, data['resource_type'], data['resource_id']
    )
    return jsonify({'detached': removed})


# -------------------------------------------------------------- resolved

@shared_resources_bp.route('/resolved', methods=['GET'])
@jwt_required()
def resolved():
    """Effective merged variables for a resource (secrets masked)."""
    resource_type = request.args.get('resource_type')
    resource_id = request.args.get('resource_id')
    if not resource_type or resource_id in (None, ''):
        return _bad('resource_type and resource_id are required')

    variables = SharedResourceService.resolve_variables(
        resource_type, resource_id, mask_secrets=True
    )
    groups = SharedResourceService.list_attached_groups(resource_type, resource_id)
    payload = {
        'resource_type': resource_type,
        'resource_id': str(resource_id),
        'variables': variables,
        'groups': [g.to_dict() for g in groups],
    }
    return jsonify(mask_sensitive(payload))


@shared_resources_bp.route('/resolved/hierarchical', methods=['GET'])
@jwt_required()
def resolved_hierarchical():
    """Hierarchical effective variables for a resource (secrets masked).

    Merges scope-inherited groups with the resource's directly-attached groups.
    Precedence, lowest → highest:

        workspace < project < environment < direct attachments

    Each returned variable carries a ``source_scope`` provenance marker. Pass the
    scope ids via ``?workspace_id=&project_id=&environment_id=``; any omitted
    scope simply contributes no layer.
    """
    resource_type = request.args.get('resource_type')
    resource_id = request.args.get('resource_id')
    if not resource_type or resource_id in (None, ''):
        return _bad('resource_type and resource_id are required')

    context = {
        'workspace_id': request.args.get('workspace_id'),
        'project_id': request.args.get('project_id'),
        'environment_id': request.args.get('environment_id'),
    }
    variables = SharedResourceService.resolve_hierarchical(
        resource_type, resource_id, context=context, mask_secrets=True
    )
    payload = {
        'resource_type': resource_type,
        'resource_id': str(resource_id),
        'context': {k: v for k, v in context.items() if v not in (None, '')},
        'variables': variables,
    }
    return jsonify(mask_sensitive(payload))
