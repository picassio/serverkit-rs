"""Project / Environment hierarchy.

Adds:
- projects, environments
- applications.project_id / environment_id (nullable, opt-in)

Organizes apps under Workspace -> Project -> Environment. Columns stay nullable;
unassigned apps remain valid (no destructive backfill).

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 031_projects_environments
Revises: 030_buildpacks_snapshots
Create Date: 2026-06-23
"""
from alembic import op
import sqlalchemy as sa

revision = '031_projects_environments'
down_revision = '030_buildpacks_snapshots'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())

    if 'projects' not in existing:
        op.create_table(
            'projects',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('workspace_id', sa.Integer, nullable=False),
            sa.Column('name', sa.String(128), nullable=False),
            sa.Column('slug', sa.String(128), nullable=False),
            sa.Column('description', sa.Text, nullable=True),
            sa.Column('metadata_json', sa.Text, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.Column('updated_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.UniqueConstraint('workspace_id', 'slug', name='uq_project_workspace_slug'),
        )
        op.create_index('ix_projects_workspace_id', 'projects', ['workspace_id'])

    if 'environments' not in existing:
        op.create_table(
            'environments',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('project_id', sa.Integer, nullable=False),
            sa.Column('name', sa.String(64), nullable=False),
            sa.Column('slug', sa.String(64), nullable=False),
            sa.Column('is_default', sa.Boolean, nullable=True),
            sa.Column('order', sa.Integer, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.UniqueConstraint('project_id', 'slug', name='uq_environment_project_slug'),
        )
        op.create_index('ix_environments_project_id', 'environments', ['project_id'])

    if 'applications' in existing:
        cols = {c['name'] for c in inspector.get_columns('applications')}
        # Plain columns (FK not enforceable via SQLite ALTER ADD COLUMN; the ORM
        # still declares the relationship).
        if 'project_id' not in cols:
            op.add_column('applications', sa.Column('project_id', sa.Integer, nullable=True))
            op.create_index('ix_applications_project_id', 'applications', ['project_id'])
        if 'environment_id' not in cols:
            op.add_column('applications', sa.Column('environment_id', sa.Integer, nullable=True))
            op.create_index('ix_applications_environment_id', 'applications', ['environment_id'])


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())
    if 'applications' in existing:
        cols = {c['name'] for c in inspector.get_columns('applications')}
        for col in ('environment_id', 'project_id'):
            if col in cols:
                op.drop_column('applications', col)
    if 'environments' in existing:
        op.drop_table('environments')
    if 'projects' in existing:
        op.drop_table('projects')
