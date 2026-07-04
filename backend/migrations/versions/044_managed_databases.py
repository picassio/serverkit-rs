"""Managed databases.

Adds:
- managed_databases (durable tracking of provisioned/adopted databases so
  BackupPolicy target_id='database' has a real FK and connection strings have a
  home; live introspection is unchanged)

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 044_managed_databases
Revises: 043_app_volumes
Create Date: 2026-07-01
"""
from alembic import op
import sqlalchemy as sa

revision = '044_managed_databases'
down_revision = '043_app_volumes'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())

    if 'managed_databases' not in existing:
        op.create_table(
            'managed_databases',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('engine', sa.String(20), nullable=False),
            sa.Column('name', sa.String(200), nullable=False),
            sa.Column('host_kind', sa.String(20), nullable=False, server_default='host'),
            sa.Column('container_ref', sa.String(200), nullable=True),
            sa.Column('host', sa.String(255), nullable=False, server_default='localhost'),
            sa.Column('port', sa.Integer, nullable=True),
            sa.Column('owner_application_id', sa.Integer, nullable=True),
            sa.Column('origin', sa.String(20), nullable=False, server_default='provisioned'),
            sa.Column('admin_username', sa.String(180), nullable=True),
            sa.Column('admin_secret_encrypted', sa.Text, nullable=True),
            sa.Column('workspace_id', sa.Integer, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.Column('updated_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.ForeignKeyConstraint(['owner_application_id'], ['applications.id'], ondelete='SET NULL'),
            sa.ForeignKeyConstraint(['workspace_id'], ['workspaces.id']),
            sa.UniqueConstraint('engine', 'host', 'name', name='uq_managed_database'),
        )
        op.create_index('ix_managed_databases_engine', 'managed_databases', ['engine'])
        op.create_index('ix_managed_databases_owner_application_id', 'managed_databases', ['owner_application_id'])
        op.create_index('ix_managed_databases_workspace_id', 'managed_databases', ['workspace_id'])
        op.create_index('ix_managed_databases_created_at', 'managed_databases', ['created_at'])


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())
    if 'managed_databases' in existing:
        op.drop_table('managed_databases')
