"""Link DeploymentJob to the release ledgers (§3 unification).

Adds to deployment_jobs:
- deployment_id      (FK deployments.id)
- git_deployment_id  (FK git_deployments.id)
- webhook_id         (FK git_webhooks.id)
- commit_hash, image_tag, container_id

Makes DeploymentJob the canonical execution record that can point at the
Deployment / GitDeployment release rows it produces. All nullable, additive.

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on
boot before Alembic, so guard on the live schema. SQLite can't add FK
constraints after the fact, so the columns are added as plain integers — the
ORM-side relationships still resolve, matching how create_all builds a fresh DB.

Revision ID: 039_deployment_job_links
Revises: 038_email_provider_usage_flags
Create Date: 2026-06-24
"""
from alembic import op
import sqlalchemy as sa

revision = '039_deployment_job_links'
down_revision = '038_email_provider_usage_flags'
branch_labels = None
depends_on = None

TABLE = 'deployment_jobs'


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
    _add_col(inspector, 'deployment_id', sa.Column('deployment_id', sa.Integer(), nullable=True))
    _add_col(inspector, 'git_deployment_id', sa.Column('git_deployment_id', sa.Integer(), nullable=True))
    _add_col(inspector, 'webhook_id', sa.Column('webhook_id', sa.Integer(), nullable=True))
    _add_col(inspector, 'commit_hash', sa.Column('commit_hash', sa.String(40), nullable=True))
    _add_col(inspector, 'image_tag', sa.Column('image_tag', sa.String(255), nullable=True))
    _add_col(inspector, 'container_id', sa.Column('container_id', sa.String(100), nullable=True))


def downgrade():
    inspector = sa.inspect(op.get_bind())
    for col in ('container_id', 'image_tag', 'commit_hash', 'webhook_id',
                'git_deployment_id', 'deployment_id'):
        _drop_col(inspector, col)
