"""Per-resource access grants (#33 — per-site ACL).

A grant gives a user access to a specific resource (currently an application,
which transitively covers its WordPress site / databases / domains) without
owning it. This service is the single seam for creating, revoking, querying, and
checking grants; enforcement lives in WorkspaceService.scope_query (lists) and
the per-resource access helpers in the API layer.
"""

RESOURCE_APPLICATION = 'application'


class ResourceGrantService:

    @staticmethod
    def granted_ids(user_id, resource_type):
        """The resource_ids of `resource_type` this user has been granted."""
        from app.models.workspace import ResourceGrant
        rows = (ResourceGrant.query
                .with_entities(ResourceGrant.resource_id)
                .filter_by(user_id=user_id, resource_type=resource_type)
                .all())
        return [r[0] for r in rows]

    @staticmethod
    def user_has_grant(user_id, resource_type, resource_id):
        from app.models.workspace import ResourceGrant
        return ResourceGrant.query.filter_by(
            user_id=user_id, resource_type=resource_type, resource_id=resource_id
        ).first() is not None

    @staticmethod
    def grant_role(user_id, resource_type, resource_id):
        """The role of this user's grant on the resource ('viewer'/'editor'), or
        None if there is no grant."""
        from app.models.workspace import ResourceGrant
        row = ResourceGrant.query.filter_by(
            user_id=user_id, resource_type=resource_type, resource_id=resource_id
        ).first()
        return row.role if row else None

    # --- access helpers (the shared seam used by every app-related blueprint) ---

    @staticmethod
    def can_access_app(user, app):
        """Read access to an application: owner, admin, or ANY grant (#33 ACL).
        Uses user.id (int) — get_jwt_identity() is the stringified token id."""
        if user.is_admin or app.user_id == user.id:
            return True
        return ResourceGrantService.user_has_grant(user.id, 'application', app.id)

    @staticmethod
    def can_edit_app(user, app):
        """Write/operate access to an application: owner, admin, or an EDITOR
        grant (#33 ACL). A viewer grant confers read access only."""
        if user.is_admin or app.user_id == user.id:
            return True
        return ResourceGrantService.grant_role(user.id, 'application', app.id) == 'editor'

    @staticmethod
    def list_for_resource(resource_type, resource_id):
        from app.models.workspace import ResourceGrant
        return ResourceGrant.query.filter_by(
            resource_type=resource_type, resource_id=resource_id
        ).all()

    @staticmethod
    def grant(user_id, resource_type, resource_id, granted_by=None, role='editor'):
        """Create (or update) a grant. Idempotent on (user, resource)."""
        from app import db
        from app.models.workspace import ResourceGrant
        existing = ResourceGrant.query.filter_by(
            user_id=user_id, resource_type=resource_type, resource_id=resource_id
        ).first()
        if existing:
            existing.role = role
            db.session.commit()
            return existing
        row = ResourceGrant(user_id=user_id, resource_type=resource_type,
                            resource_id=resource_id, role=role, granted_by=granted_by)
        db.session.add(row)
        db.session.commit()
        return row

    @staticmethod
    def revoke(grant_id, resource_type=None, resource_id=None):
        """Delete a grant by id. When resource_type/resource_id are given, the
        grant must belong to that resource (so a caller can't revoke another
        resource's grant by id)."""
        from app import db
        from app.models.workspace import ResourceGrant
        row = ResourceGrant.query.get(grant_id)
        if row is None:
            return False
        if resource_type is not None and (row.resource_type != resource_type or row.resource_id != resource_id):
            return False
        db.session.delete(row)
        db.session.commit()
        return True
