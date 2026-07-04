"""DNS change activity log.

Adds:
- dns_changes

Audit trail of every record write ServerKit sends to a connected provider, powering
the "Changes to your Cloudflare" activity feed on the connection.

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 027_dns_change_log
Revises: 026_managed_dns_records
Create Date: 2026-06-22
"""
from alembic import op
import sqlalchemy as sa

revision = '027_dns_change_log'
down_revision = '026_managed_dns_records'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'dns_changes' in set(inspector.get_table_names()):
        return
    op.create_table(
        'dns_changes',
        sa.Column('id', sa.Integer, nullable=False),
        sa.Column('dns_provider_config_id', sa.Integer, nullable=True),
        sa.Column('provider', sa.String(64), nullable=False),
        sa.Column('provider_zone_id', sa.String(128), nullable=True),
        sa.Column('action', sa.String(16), nullable=False),
        sa.Column('record_type', sa.String(10), nullable=True),
        sa.Column('name', sa.String(256), nullable=True),
        sa.Column('content', sa.Text, nullable=True),
        sa.Column('provider_record_id', sa.String(128), nullable=True),
        sa.Column('source', sa.String(40), nullable=True),
        sa.Column('result', sa.String(16), nullable=False),
        sa.Column('error', sa.Text, nullable=True),
        sa.Column('created_at', sa.DateTime, nullable=True),
        sa.PrimaryKeyConstraint('id'),
    )
    op.create_index('ix_dns_changes_dns_provider_config_id', 'dns_changes', ['dns_provider_config_id'])
    op.create_index('ix_dns_changes_provider_zone_id', 'dns_changes', ['provider_zone_id'])
    op.create_index('ix_dns_changes_created_at', 'dns_changes', ['created_at'])


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'dns_changes' not in set(inspector.get_table_names()):
        return
    op.drop_table('dns_changes')
