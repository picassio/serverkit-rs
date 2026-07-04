"""Role-Based Access Control middleware and decorators."""
from functools import wraps
from flask import g, jsonify
from flask_jwt_extended import jwt_required, get_jwt_identity, verify_jwt_in_request
from app.models import User


def get_current_user():
    """Get the current authenticated user (via API key or JWT)."""
    # Check API key auth first
    api_key_user = getattr(g, 'api_key_user', None)
    if api_key_user:
        return api_key_user

    # Fall back to JWT
    user_id = get_jwt_identity()
    if user_id:
        return User.query.get(user_id)
    return None


def auth_required():
    """
    Decorator that accepts either API key or JWT authentication.
    Use this instead of @jwt_required() to support both auth methods.
    """
    def decorator(fn):
        @wraps(fn)
        def wrapper(*args, **kwargs):
            # If API key already authenticated via middleware, proceed
            if getattr(g, 'api_key_user', None):
                return fn(*args, **kwargs)
            # Otherwise require JWT
            verify_jwt_in_request()
            return fn(*args, **kwargs)
        return wrapper
    return decorator


def require_role(*allowed_roles):
    """
    Decorator that requires the user to have one of the specified roles.

    Usage:
        @require_role('admin', 'developer')
        def my_endpoint():
            ...
    """
    def decorator(fn):
        @wraps(fn)
        @auth_required()
        def wrapper(*args, **kwargs):
            user = get_current_user()
            if not user:
                return jsonify({'error': 'User not found'}), 404
            if not user.is_active:
                return jsonify({'error': 'Account is deactivated'}), 403
            if user.role not in allowed_roles:
                return jsonify({'error': 'Insufficient permissions'}), 403
            return fn(*args, **kwargs)
        return wrapper
    return decorator


def admin_required(fn):
    """
    Decorator that requires admin role.

    Usage:
        @admin_required
        def admin_only_endpoint():
            ...
    """
    @wraps(fn)
    @auth_required()
    def wrapper(*args, **kwargs):
        user = get_current_user()
        if not user:
            return jsonify({'error': 'User not found'}), 404
        if not user.is_active:
            return jsonify({'error': 'Account is deactivated'}), 403
        if not user.is_admin:
            return jsonify({'error': 'Admin access required'}), 403
        return fn(*args, **kwargs)
    return wrapper


def developer_required(fn):
    """
    Decorator that requires developer role or higher (admin or developer).

    Usage:
        @developer_required
        def developer_endpoint():
            ...
    """
    @wraps(fn)
    @auth_required()
    def wrapper(*args, **kwargs):
        user = get_current_user()
        if not user:
            return jsonify({'error': 'User not found'}), 404
        if not user.is_active:
            return jsonify({'error': 'Account is deactivated'}), 403
        if not user.is_developer:
            return jsonify({'error': 'Developer access required'}), 403
        return fn(*args, **kwargs)
    return wrapper


def permission_required(feature, level='read'):
    """
    Decorator that checks per-feature permissions.

    Usage:
        @permission_required('applications', 'write')
        def create_app():
            ...
    """
    def decorator(fn):
        @wraps(fn)
        @auth_required()
        def wrapper(*args, **kwargs):
            user = get_current_user()
            if not user:
                return jsonify({'error': 'User not found'}), 404
            if not user.is_active:
                return jsonify({'error': 'Account is deactivated'}), 403
            if not user.has_permission(feature, level):
                return jsonify({
                    'error': f'Permission denied: {level} access to {feature} required'
                }), 403
            return fn(*args, **kwargs)
        return wrapper
    return decorator


def viewer_required(fn):
    """
    Decorator that requires viewer role or higher (any valid role).
    Essentially just requires authentication and active account.

    Usage:
        @viewer_required
        def viewer_endpoint():
            ...
    """
    @wraps(fn)
    @auth_required()
    def wrapper(*args, **kwargs):
        user = get_current_user()
        if not user:
            return jsonify({'error': 'User not found'}), 404
        if not user.is_active:
            return jsonify({'error': 'Account is deactivated'}), 403
        if not user.is_viewer:
            return jsonify({'error': 'Access denied'}), 403
        return fn(*args, **kwargs)
    return wrapper
