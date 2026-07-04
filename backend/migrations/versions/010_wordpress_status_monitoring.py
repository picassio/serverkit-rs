"""Link status components to managed WordPress sites + incidents to components (#26).

Adds:
- status_components.wordpress_site_id (nullable FK -> wordpress_sites.id)
- status_incidents.component_id       (nullable FK -> status_components.id)

These let a managed WordPress site appear on a status page as a health-driven
component (real uptime %) and let an auto-opened incident be auto-resolved on
recovery.

Idempotent: MigrationService._fix_missing_columns may have already added these
columns on boot before Alembic runs, so guard on the live schema. Columns are
added as plain INTEGERs (no enforced FK), matching the auto-add path and the
existing migration style.

Revision ID: 010_wordpress_status_monitoring
Revises: 009_wordpress_site_tags
Create Date: 2026-06-01
"""
from alembic import op
import sqlalchemy as sa

revision = '010_wordpress_status_monitoring'
down_revision = '009_wordpress_site_tags'
branch_labels = None
depends_on = None


def _has_col(inspector, table, col):
    if table not in inspector.get_table_names():
        return False
    return col in {c['name'] for c in inspector.get_columns(table)}


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'status_components' in inspector.get_table_names() and \
            not _has_col(inspector, 'status_components', 'wordpress_site_id'):
        with op.batch_alter_table('status_components') as batch_op:
            batch_op.add_column(sa.Column('wordpress_site_id', sa.Integer(), nullable=True))
    if 'status_incidents' in inspector.get_table_names() and \
            not _has_col(inspector, 'status_incidents', 'component_id'):
        with op.batch_alter_table('status_incidents') as batch_op:
            batch_op.add_column(sa.Column('component_id', sa.Integer(), nullable=True))


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if _has_col(inspector, 'status_incidents', 'component_id'):
        with op.batch_alter_table('status_incidents') as batch_op:
            batch_op.drop_column('component_id')
    if _has_col(inspector, 'status_components', 'wordpress_site_id'):
        with op.batch_alter_table('status_components') as batch_op:
            batch_op.drop_column('wordpress_site_id')
