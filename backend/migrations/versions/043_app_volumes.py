"""Per-app managed volumes.

Adds:
- app_volumes (first-class tracked persistent volumes attached to an application,
  replacing fragile relative bind mounts)

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 043_app_volumes
Revises: 042_container_registries
Create Date: 2026-07-01
"""
from alembic import op
import sqlalchemy as sa

revision = '043_app_volumes'
down_revision = '042_container_registries'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())

    if 'app_volumes' not in existing:
        op.create_table(
            'app_volumes',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('application_id', sa.Integer, nullable=False),
            sa.Column('name', sa.String(120), nullable=False),
            sa.Column('docker_volume_name', sa.String(200), nullable=False),
            sa.Column('mount_path', sa.String(500), nullable=False),
            sa.Column('driver', sa.String(40), nullable=False, server_default='local'),
            sa.Column('read_only', sa.Boolean, nullable=False, server_default=sa.false()),
            sa.Column('size_bytes', sa.BigInteger, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.Column('updated_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.ForeignKeyConstraint(['application_id'], ['applications.id'], ondelete='CASCADE'),
            sa.UniqueConstraint('docker_volume_name', name='uq_app_volume_docker_name'),
            sa.UniqueConstraint('application_id', 'mount_path', name='uq_app_volume_mount'),
        )
        op.create_index('ix_app_volumes_application_id', 'app_volumes', ['application_id'])


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())
    if 'app_volumes' in existing:
        op.drop_table('app_volumes')
