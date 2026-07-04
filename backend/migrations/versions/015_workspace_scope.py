"""Workspace scoping foundation (#33 — agency-scale core).

Adds:
- applications.workspace_id (nullable FK -> workspaces.id)
- servers.workspace_id      (nullable FK -> workspaces.id)

Backfill (idempotent, raw SQL like 006_default_full_permissions):
- ensure a "Default" workspace exists,
- attach every workspace-less application/server to it,
- make every existing user a member of it, so nobody loses visibility once
  workspace scoping is activated.

Scoping itself is opt-in (a request only filters by workspace when it carries a
workspace context), so this migration changes no behavior on its own — it only
gives every resource a workspace home.

Idempotent: MigrationService._fix_missing_columns runs db.create_all() on boot
before Alembic (creating the workspaces tables + adding the columns), so guard
on the live schema exactly like 009..014.

Revision ID: 015_workspace_scope
Revises: 014_wordpress_reports
Create Date: 2026-06-01
"""
from datetime import datetime

from alembic import op
import sqlalchemy as sa

revision = '015_workspace_scope'
down_revision = '014_wordpress_reports'
branch_labels = None
depends_on = None


def _has_col(inspector, table, col):
    if table not in inspector.get_table_names():
        return False
    return col in {c['name'] for c in inspector.get_columns(table)}


def _ensure_index(inspector, name, table, col):
    """Create an index idempotently (matches SQLAlchemy's ix_<table>_<col> name,
    so a fresh create_all install — which already has it — isn't duplicated)."""
    if not _has_col(inspector, table, col):
        return
    if name not in {ix['name'] for ix in inspector.get_indexes(table)}:
        op.create_index(name, table, [col])


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    tables = set(inspector.get_table_names())

    # The FK target (workspaces) is created by db.create_all() on boot, not by any
    # migration — guard on it so a standalone `alembic upgrade` (e.g. Postgres,
    # workspaces absent) doesn't fail adding a FK to a missing table.
    if 'workspaces' in tables:
        if 'applications' in tables and not _has_col(inspector, 'applications', 'workspace_id'):
            with op.batch_alter_table('applications') as batch_op:
                batch_op.add_column(sa.Column('workspace_id', sa.Integer(),
                                              sa.ForeignKey('workspaces.id'), nullable=True))
        if 'servers' in tables and not _has_col(inspector, 'servers', 'workspace_id'):
            with op.batch_alter_table('servers') as batch_op:
                batch_op.add_column(sa.Column('workspace_id', sa.Integer(),
                                              sa.ForeignKey('workspaces.id'), nullable=True))

    # Indexes: the models declare index=True, but neither _fix_missing_columns' raw
    # ADD COLUMN nor batch add_column creates an index — so upgraded installs would
    # silently lack it. Create idempotently (re-inspect to see the just-added column).
    inspector = sa.inspect(conn)
    _ensure_index(inspector, 'ix_applications_workspace_id', 'applications', 'workspace_id')
    _ensure_index(inspector, 'ix_servers_workspace_id', 'servers', 'workspace_id')

    # Backfill needs the workspace tables (normally created by db.create_all on boot).
    if 'workspaces' in tables and 'workspace_members' in tables:
        _backfill(conn)


def _backfill(conn):
    now = datetime.utcnow()

    # 1. Find-or-create the Default workspace (quota columns set to 0 so the
    #    model's int math — e.g. `ws.max_users > 0` — never sees NULL).
    row = conn.execute(sa.text(
        "SELECT id FROM workspaces WHERE slug = :slug"), {'slug': 'default'}).fetchone()
    if row:
        ws_id = row[0]
    else:
        conn.execute(sa.text(
            "INSERT INTO workspaces "
            "(name, slug, description, status, max_servers, max_users, max_api_calls, created_at, updated_at) "
            "VALUES ('Default', 'default', :descr, 'active', 0, 0, 0, :now, :now)"),
            {'descr': 'Default workspace (auto-created for existing resources).', 'now': now})
        ws_id = conn.execute(sa.text(
            "SELECT id FROM workspaces WHERE slug = :slug"), {'slug': 'default'}).fetchone()[0]

    # 2. Attach workspace-less resources to the default workspace.
    for table in ('applications', 'servers'):
        conn.execute(sa.text(
            f"UPDATE {table} SET workspace_id = :ws WHERE workspace_id IS NULL"), {'ws': ws_id})

    # 3. Make every existing user a member of the default workspace (idempotent).
    if 'users' in set(sa.inspect(conn).get_table_names()):
        conn.execute(sa.text(
            "INSERT INTO workspace_members (workspace_id, user_id, role, joined_at) "
            "SELECT :ws, u.id, 'member', :now FROM users u "
            "WHERE NOT EXISTS ("
            "  SELECT 1 FROM workspace_members m WHERE m.workspace_id = :ws AND m.user_id = u.id)"),
            {'ws': ws_id, 'now': now})


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    for name, table in (('ix_servers_workspace_id', 'servers'),
                        ('ix_applications_workspace_id', 'applications')):
        if table in inspector.get_table_names() and name in {ix['name'] for ix in inspector.get_indexes(table)}:
            op.drop_index(name, table_name=table)
    for table in ('servers', 'applications'):
        if _has_col(inspector, table, 'workspace_id'):
            with op.batch_alter_table(table) as batch_op:
                batch_op.drop_column('workspace_id')
