"""Email provider connections (Notification Bus transports).

Adds:
- email_provider_connections

Idempotent: MigrationService runs db.create_all() on boot before Alembic,
so guard on the live schema.

Revision ID: 024_email_providers
Revises: 023_notification_bus
Create Date: 2026-06-21
"""
from alembic import op
import sqlalchemy as sa

revision = '024_email_providers'
down_revision = '023_notification_bus'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    tables = set(inspector.get_table_names())

    if 'email_provider_connections' not in tables:
        op.create_table(
            'email_provider_connections',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('provider', sa.String(40), nullable=False),
            sa.Column('name', sa.String(120), nullable=False),
            sa.Column('credentials_json', sa.Text, nullable=True),
            sa.Column('from_address', sa.String(255), nullable=True),
            sa.Column('from_name', sa.String(120), nullable=True),
            sa.Column('is_default', sa.Boolean, nullable=True, server_default=sa.false()),
            sa.Column('is_active', sa.Boolean, nullable=True, server_default=sa.true()),
            sa.Column('created_by', sa.Integer, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.Column('updated_at', sa.DateTime, nullable=True),
            sa.Column('last_tested_at', sa.DateTime, nullable=True),
            sa.Column('last_test_ok', sa.Boolean, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.ForeignKeyConstraint(['created_by'], ['users.id']),
        )
        op.create_index('ix_email_provider_connections_is_default',
                        'email_provider_connections', ['is_default'])


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    tables = set(inspector.get_table_names())
    if 'email_provider_connections' in tables:
        op.drop_table('email_provider_connections')
