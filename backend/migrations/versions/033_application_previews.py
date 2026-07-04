"""PR preview environments.

Adds:
- application_previews (one row per PR preview)
- application_preview_settings (per-app enable + domain template + TTL)

Backs auto-deploying a PR branch to an isolated, disposable URL that is torn
down when the PR closes (apps and WordPress sites).

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 033_application_previews
Revises: 032_shared_resources
Create Date: 2026-06-23
"""
from alembic import op
import sqlalchemy as sa

revision = '033_application_previews'
down_revision = '032_shared_resources'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())

    if 'application_previews' not in existing:
        op.create_table(
            'application_previews',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('application_id', sa.Integer, nullable=False),
            sa.Column('pr_number', sa.Integer, nullable=True),
            sa.Column('pr_title', sa.String(500), nullable=True),
            sa.Column('branch', sa.String(255), nullable=True),
            sa.Column('status', sa.String(20), nullable=True),
            sa.Column('domain', sa.String(255), nullable=True),
            sa.Column('container_ids', sa.Text, nullable=True),
            sa.Column('commit_sha', sa.String(64), nullable=True),
            sa.Column('expires_at', sa.DateTime, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.Column('updated_at', sa.DateTime, nullable=True),
            sa.Column('deleted_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.ForeignKeyConstraint(['application_id'], ['applications.id']),
        )
        op.create_index('ix_application_previews_application_id', 'application_previews', ['application_id'])

    if 'application_preview_settings' not in existing:
        op.create_table(
            'application_preview_settings',
            sa.Column('application_id', sa.Integer, nullable=False),
            sa.Column('enabled', sa.Boolean, nullable=True),
            sa.Column('domain_template', sa.String(255), nullable=True),
            sa.Column('target_server_id', sa.String(36), nullable=True),
            sa.Column('ttl_days', sa.Integer, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.Column('updated_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('application_id'),
            sa.ForeignKeyConstraint(['application_id'], ['applications.id']),
        )


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())
    for tbl in ('application_preview_settings', 'application_previews'):
        if tbl in existing:
            op.drop_table(tbl)
