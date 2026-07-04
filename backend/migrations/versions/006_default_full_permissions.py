"""Grant ['*'] to servers stuck with empty permissions.

Single-tenant deployments registered servers with the prior default
of `permissions = []`, which made every action 403. The new default
in api/servers.py is `['*']`. This migration retroactively fixes
already-registered servers — but only when their permissions list
is empty / NULL, so admin-defined ACLs are left untouched.

Revision ID: 006_default_full_permissions
Revises: 005_server_capabilities_cache
Create Date: 2026-05-01
"""

from alembic import op
import sqlalchemy as sa
import json

revision = '006_default_full_permissions'
down_revision = '005_server_capabilities_cache'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'servers' not in inspector.get_table_names():
        return
    cols = {c['name'] for c in inspector.get_columns('servers')}
    if 'permissions' not in cols:
        return

    # Iterate rows; SQLAlchemy's JSON column round-trips as text in some
    # backends. Normalize to a list, check emptiness, write back.
    rows = conn.execute(sa.text("SELECT id, permissions FROM servers")).fetchall()
    for row in rows:
        raw = row.permissions
        try:
            current = json.loads(raw) if isinstance(raw, str) else (raw or [])
        except (ValueError, TypeError):
            current = []
        if not isinstance(current, list):
            current = []
        if len(current) == 0:
            # Stuck row — grant full access. Admin-curated lists with
            # any entry are untouched.
            conn.execute(
                sa.text("UPDATE servers SET permissions = :p WHERE id = :id"),
                {'p': json.dumps(['*']), 'id': row.id},
            )


def downgrade():
    # Intentionally a no-op — we can't tell which `['*']` rows were
    # originally empty vs. explicitly set. Revert manually if needed.
    pass
