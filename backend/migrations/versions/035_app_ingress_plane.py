"""App ingress plane.

Adds:
- applications.ingress_plane ('nginx' | 'proxy_stack', NULL = default nginx)

Makes the reverse-proxy boundary explicit per app so the UI can warn when a
server's configured proxy stack disagrees with the apps running on it.

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 035_app_ingress_plane
Revises: 034_proxy_stacks
Create Date: 2026-06-23
"""
from alembic import op
import sqlalchemy as sa

revision = '035_app_ingress_plane'
down_revision = '034_proxy_stacks'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())
    if 'applications' in existing:
        cols = {c['name'] for c in inspector.get_columns('applications')}
        if 'ingress_plane' not in cols:
            op.add_column('applications', sa.Column('ingress_plane', sa.String(20), nullable=True))


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())
    if 'applications' in existing:
        cols = {c['name'] for c in inspector.get_columns('applications')}
        if 'ingress_plane' in cols:
            op.drop_column('applications', 'ingress_plane')
