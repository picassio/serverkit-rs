"""Generalize BackupPolicy to all data-protection targets (§8 unification).

Adds:
- backup_policies.target_subtype  (e.g. 'mysql' for a database target)
- backup_policies.target_meta_json (per-target details: db descriptor, path list)

These let the single BackupPolicy/BackupRun system cover database, files, and
server targets in addition to application/wordpress_site, folding in the legacy
BackupService responsibilities.

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on
boot before Alembic, so guard on the live schema.

Revision ID: 037_backup_target_types
Revises: 036_env_target_service
Create Date: 2026-06-24
"""
from alembic import op
import sqlalchemy as sa

revision = '037_backup_target_types'
down_revision = '036_env_target_service'
branch_labels = None
depends_on = None


def _add_col(inspector, table, col, column):
    if table in inspector.get_table_names():
        cols = {c['name'] for c in inspector.get_columns(table)}
        if col not in cols:
            op.add_column(table, column)


def _drop_col(inspector, table, col):
    if table in inspector.get_table_names():
        cols = {c['name'] for c in inspector.get_columns(table)}
        if col in cols:
            op.drop_column(table, col)


def upgrade():
    inspector = sa.inspect(op.get_bind())
    _add_col(inspector, 'backup_policies', 'target_subtype',
             sa.Column('target_subtype', sa.String(40), nullable=True))
    _add_col(inspector, 'backup_policies', 'target_meta_json',
             sa.Column('target_meta_json', sa.Text(), nullable=True, server_default='{}'))


def downgrade():
    inspector = sa.inspect(op.get_bind())
    _drop_col(inspector, 'backup_policies', 'target_meta_json')
    _drop_col(inspector, 'backup_policies', 'target_subtype')
