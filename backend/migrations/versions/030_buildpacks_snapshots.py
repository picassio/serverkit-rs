"""Build packs + deployment config snapshots.

Adds:
- applications.buildpack_type / buildpack_plan / buildpack_overrides
- deployment_snapshots (immutable resolved-config snapshot per deploy, with hash)

Backs zero-Dockerfile build-pack deploys (detected plan persisted on the app) and
the deployment config snapshot + diff/restore engine.

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 030_buildpacks_snapshots
Revises: 029_server_onboarding
Create Date: 2026-06-23
"""
from alembic import op
import sqlalchemy as sa

revision = '030_buildpacks_snapshots'
down_revision = '029_server_onboarding'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())

    if 'applications' in existing:
        cols = {c['name'] for c in inspector.get_columns('applications')}
        if 'buildpack_type' not in cols:
            op.add_column('applications', sa.Column('buildpack_type', sa.String(20), nullable=True))
        if 'buildpack_plan' not in cols:
            op.add_column('applications', sa.Column('buildpack_plan', sa.Text, nullable=True))
        if 'buildpack_overrides' not in cols:
            op.add_column('applications', sa.Column('buildpack_overrides', sa.Text, nullable=True))

    if 'deployment_snapshots' not in existing:
        op.create_table(
            'deployment_snapshots',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('application_id', sa.Integer, nullable=False),
            sa.Column('deployment_id', sa.Integer, nullable=True),
            sa.Column('snapshot_hash', sa.String(64), nullable=False),
            sa.Column('config_json', sa.Text, nullable=False, server_default='{}'),
            sa.Column('summary', sa.String(255), nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.ForeignKeyConstraint(['application_id'], ['applications.id']),
            sa.ForeignKeyConstraint(['deployment_id'], ['deployments.id']),
        )
        op.create_index('ix_deployment_snapshots_application_id', 'deployment_snapshots', ['application_id'])
        op.create_index('ix_deployment_snapshots_deployment_id', 'deployment_snapshots', ['deployment_id'])


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())
    if 'deployment_snapshots' in existing:
        op.drop_table('deployment_snapshots')
    if 'applications' in existing:
        cols = {c['name'] for c in inspector.get_columns('applications')}
        for col in ('buildpack_overrides', 'buildpack_plan', 'buildpack_type'):
            if col in cols:
                op.drop_column('applications', col)
