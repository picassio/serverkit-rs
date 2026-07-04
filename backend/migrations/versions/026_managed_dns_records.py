"""DNS record ownership ledger.

Adds:
- managed_dns_records

The single source of truth for "this provider DNS record was created by ServerKit",
so the never-touch-foreign guard and the zone mirror can tell our records apart from
the user's own. Written by every connected-provider write path; removed on delete.

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 026_managed_dns_records
Revises: 025_dns_provider_link
Create Date: 2026-06-22
"""
from alembic import op
import sqlalchemy as sa

revision = '026_managed_dns_records'
down_revision = '025_dns_provider_link'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'managed_dns_records' in set(inspector.get_table_names()):
        return
    op.create_table(
        'managed_dns_records',
        sa.Column('id', sa.Integer, nullable=False),
        sa.Column('dns_provider_config_id', sa.Integer, nullable=True),
        sa.Column('provider', sa.String(64), nullable=False),
        sa.Column('provider_zone_id', sa.String(128), nullable=False),
        sa.Column('provider_record_id', sa.String(128), nullable=True),
        sa.Column('record_type', sa.String(10), nullable=False),
        sa.Column('name', sa.String(256), nullable=False),
        sa.Column('content', sa.Text, nullable=True),
        sa.Column('source', sa.String(40), nullable=True),
        sa.Column('app_id', sa.Integer, nullable=True),
        sa.Column('created_at', sa.DateTime, nullable=True),
        sa.Column('updated_at', sa.DateTime, nullable=True),
        sa.PrimaryKeyConstraint('id'),
    )
    op.create_index('ix_managed_dns_records_provider_zone_id',
                    'managed_dns_records', ['provider_zone_id'])
    op.create_index('ix_managed_dns_records_provider_record_id',
                    'managed_dns_records', ['provider_record_id'])


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'managed_dns_records' not in set(inspector.get_table_names()):
        return
    op.drop_table('managed_dns_records')
