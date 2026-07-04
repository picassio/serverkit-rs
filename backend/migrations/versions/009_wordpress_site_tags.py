"""Add tags column to wordpress_sites for agency organization.

Stored as JSON-encoded TEXT (a JSON array of strings), matching the
existing sync_config / resource_limits / git_paths columns.

Idempotent: MigrationService._fix_missing_columns may have already added
this column on boot before Alembic runs, so guard on the live schema.

Revision ID: 009_wordpress_site_tags
Revises: 008_fix_server_metrics_id_type
Create Date: 2026-05-31
"""
from alembic import op
import sqlalchemy as sa

revision = '009_wordpress_site_tags'
down_revision = '008_fix_server_metrics_id_type'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'wordpress_sites' not in inspector.get_table_names():
        # Fresh install: create_all() produces the column from the model.
        return
    existing_cols = {c['name'] for c in inspector.get_columns('wordpress_sites')}
    if 'tags' not in existing_cols:
        with op.batch_alter_table('wordpress_sites') as batch_op:
            batch_op.add_column(sa.Column('tags', sa.Text(), nullable=True))


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'wordpress_sites' not in inspector.get_table_names():
        return
    existing_cols = {c['name'] for c in inspector.get_columns('wordpress_sites')}
    if 'tags' in existing_cols:
        with op.batch_alter_table('wordpress_sites') as batch_op:
            batch_op.drop_column('tags')
