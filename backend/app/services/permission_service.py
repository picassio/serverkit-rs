"""Service for managing per-feature user permissions."""
from app import db
from app.models import User


class PermissionService:
    """Stateless service for user permission operations."""

    @staticmethod
    def get_role_template(role):
        """Return default permissions for a role."""
        return User.ROLE_PERMISSION_TEMPLATES.get(role, {})

    @staticmethod
    def get_user_permissions(user_id):
        """Get resolved permissions for a user."""
        user = User.query.get(user_id)
        if not user:
            return None
        return user.get_permissions()

    @staticmethod
    def update_user_permissions(user_id, permissions):
        """Validate and store custom permissions for a user."""
        user = User.query.get(user_id)
        if not user:
            return {'success': False, 'error': 'User not found'}

        if user.role == User.ROLE_ADMIN:
            return {'success': False, 'error': 'Admin permissions cannot be customized'}

        error = PermissionService.validate_permissions(permissions)
        if error:
            return {'success': False, 'error': error}

        user.set_permissions(permissions)
        db.session.commit()
        return {'success': True, 'permissions': user.get_permissions()}

    @staticmethod
    def reset_to_role_defaults(user_id):
        """Clear custom permissions so user falls back to role template."""
        user = User.query.get(user_id)
        if not user:
            return {'success': False, 'error': 'User not found'}

        user.permissions = None
        db.session.commit()
        return {'success': True, 'permissions': user.get_permissions()}

    @staticmethod
    def validate_permissions(permissions):
        """Validate permission structure. Returns error string or None."""
        if not isinstance(permissions, dict):
            return 'Permissions must be an object'

        for feature, access in permissions.items():
            if feature not in User.PERMISSION_FEATURES:
                return f'Unknown feature: {feature}'
            if not isinstance(access, dict):
                return f'Feature "{feature}" must have read/write object'
            for key in access:
                if key not in ('read', 'write'):
                    return f'Invalid permission key "{key}" for feature "{feature}"'
                if not isinstance(access[key], bool):
                    return f'Permission values must be boolean for "{feature}.{key}"'
            # Write without read is invalid
            if access.get('write') and not access.get('read'):
                return f'Cannot have write without read for "{feature}"'

        return None
