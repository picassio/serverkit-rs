"""Workspace-scope secrets & webhooks (#33 follow-up).

Adds:
- secret_vaults.workspace_id     (nullable FK -> workspaces.id)
- webhook_endpoints.workspace_id (nullable FK -> workspaces.id)

Backfill (idempotent, raw SQL like 015_workspace_scope): attach every
workspace-less vault/endpoint to the "Default" workspace, so nothing disappears
once secrets scoping is active.

Scoping itself is opt-in (a request only filters by workspace when it carries a
workspace context), so this migration changes no behavior on its own — it only
gives every vault/endpoint a workspace home.

Idempotent: MigrationService runs db.create_all() on boot before Alembic
(a fresh install already has the columns + indexes from the models), so guard
on the live schema exactly like 015.

Revision ID: 021_secrets_webhooks_workspace
Revises: 020_secrets_webhooks
Create Date: 2026-06-20
"""
from alembic import op
import sqlalchemy as sa

revision = '021_secrets_webhooks_workspace'
down_revision = '020_secrets_webhooks'
branch_labels = None
depends_on = None

_TABLES = ('secret_vaults', 'webhook_endpoints')


def _has_col(inspector, table, col):
    if table not in inspector.get_table_names():
        return False
    return col in {c['name'] for c in inspector.get_columns(table)}


def _ensure_index(inspector, name, table, col):
    """Create the model's ix_<table>_<col> index idempotently (a fresh
    create_all install already has it; an ADD COLUMN upgrade would not)."""
    if not _has_col(inspector, table, col):
        return
    if name not in {ix['name'] for ix in inspector.get_indexes(table)}:
        op.create_index(name, table, [col])


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    tables = set(inspector.get_table_names())

    # The FK target (workspaces) is created by db.create_all() on boot, not by a
    # migration — guard on it so a standalone `alembic upgrade` (workspaces
    # absent) doesn't fail adding a FK to a missing table.
    if 'workspaces' in tables:
        for table in _TABLES:
            if table in tables and not _has_col(inspector, table, 'workspace_id'):
                with op.batch_alter_table(table) as batch_op:
                    batch_op.add_column(sa.Column('workspace_id', sa.Integer(),
                                                  sa.ForeignKey('workspaces.id'), nullable=True))

    inspector = sa.inspect(conn)
    for table in _TABLES:
        _ensure_index(inspector, f'ix_{table}_workspace_id', table, 'workspace_id')

    if 'workspaces' in tables:
        _backfill(conn)


def _backfill(conn):
    row = conn.execute(sa.text(
        "SELECT id FROM workspaces WHERE slug = :slug"), {'slug': 'default'}).fetchone()
    if not row:
        # 015_workspace_scope creates the Default workspace; without it there is
        # nothing to attach to, so leave rows unscoped (workspace_id IS NULL).
        return
    ws_id = row[0]
    tables = set(sa.inspect(conn).get_table_names())
    for table in _TABLES:
        if table in tables:
            conn.execute(sa.text(
                f"UPDATE {table} SET workspace_id = :ws WHERE workspace_id IS NULL"), {'ws': ws_id})


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    for table in _TABLES:
        name = f'ix_{table}_workspace_id'
        if table in inspector.get_table_names() and name in {ix['name'] for ix in inspector.get_indexes(table)}:
            op.drop_index(name, table_name=table)
    for table in _TABLES:
        if _has_col(inspector, table, 'workspace_id'):
            with op.batch_alter_table(table) as batch_op:
                batch_op.drop_column('workspace_id')
