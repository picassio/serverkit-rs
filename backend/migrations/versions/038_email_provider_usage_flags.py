"""Email provider usage flags (§6 unification).

Adds to email_provider_connections:
- uses_notifications (bool, default true)
- uses_relay         (bool, default false)
- relay_priority     (int,  default 0)

So a single connection can drive both the Notification Bus and the Postfix
outbound relay, retiring the separate single-row email_relay_config.

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on
boot before Alembic, so guard on the live schema.

Revision ID: 038_email_provider_usage_flags
Revises: 037_backup_target_types
Create Date: 2026-06-24
"""
from alembic import op
import sqlalchemy as sa

revision = '038_email_provider_usage_flags'
down_revision = '037_backup_target_types'
branch_labels = None
depends_on = None

TABLE = 'email_provider_connections'


def _add_col(inspector, col, column):
    if TABLE in inspector.get_table_names():
        cols = {c['name'] for c in inspector.get_columns(TABLE)}
        if col not in cols:
            op.add_column(TABLE, column)


def _drop_col(inspector, col):
    if TABLE in inspector.get_table_names():
        cols = {c['name'] for c in inspector.get_columns(TABLE)}
        if col in cols:
            op.drop_column(TABLE, col)


def upgrade():
    inspector = sa.inspect(op.get_bind())
    _add_col(inspector, 'uses_notifications',
             sa.Column('uses_notifications', sa.Boolean(), nullable=False, server_default=sa.true()))
    _add_col(inspector, 'uses_relay',
             sa.Column('uses_relay', sa.Boolean(), nullable=False, server_default=sa.false()))
    _add_col(inspector, 'relay_priority',
             sa.Column('relay_priority', sa.Integer(), nullable=False, server_default='0'))


def downgrade():
    inspector = sa.inspect(op.get_bind())
    _drop_col(inspector, 'relay_priority')
    _drop_col(inspector, 'uses_relay')
    _drop_col(inspector, 'uses_notifications')
