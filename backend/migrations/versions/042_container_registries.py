"""Container registry authentication.

Adds:
- container_registries (stored, Fernet-encrypted credentials for private image pulls)
- applications.registry_id (nullable FK — authenticate before pulling docker_image)

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 042_container_registries
Revises: 041_wordpress_plugin_library
Create Date: 2026-07-01
"""
from alembic import op
import sqlalchemy as sa

revision = '042_container_registries'
down_revision = '041_wordpress_plugin_library'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())

    if 'container_registries' not in existing:
        op.create_table(
            'container_registries',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('name', sa.String(180), nullable=False),
            sa.Column('provider', sa.String(40), nullable=False, server_default='generic'),
            sa.Column('registry_url', sa.String(255), nullable=True),
            sa.Column('username', sa.String(180), nullable=True),
            sa.Column('secret_encrypted', sa.Text, nullable=True),
            sa.Column('workspace_id', sa.Integer, nullable=True),
            sa.Column('created_by', sa.Integer, nullable=True),
            sa.Column('last_used_at', sa.DateTime, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.Column('updated_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.ForeignKeyConstraint(['workspace_id'], ['workspaces.id']),
            sa.ForeignKeyConstraint(['created_by'], ['users.id']),
        )
        op.create_index('ix_container_registries_provider', 'container_registries', ['provider'])
        op.create_index('ix_container_registries_workspace_id', 'container_registries', ['workspace_id'])
        op.create_index('ix_container_registries_created_at', 'container_registries', ['created_at'])

    if 'applications' in existing:
        cols = {c['name'] for c in inspector.get_columns('applications')}
        if 'registry_id' not in cols:
            op.add_column('applications', sa.Column('registry_id', sa.Integer, nullable=True))
            op.create_index('ix_applications_registry_id', 'applications', ['registry_id'])


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())

    if 'applications' in existing:
        cols = {c['name'] for c in inspector.get_columns('applications')}
        if 'registry_id' in cols:
            try:
                op.drop_index('ix_applications_registry_id', table_name='applications')
            except Exception:
                pass
            op.drop_column('applications', 'registry_id')

    if 'container_registries' in existing:
        op.drop_table('container_registries')
