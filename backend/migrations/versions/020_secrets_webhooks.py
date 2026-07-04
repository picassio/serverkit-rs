"""Secrets manager and inbound webhook gateway.

Adds:
- secret_vaults table
- secrets table
- webhook_endpoints table
- webhook_deliveries table

Revision ID: 020_secrets_webhooks
Revises: 019_passkey_credentials
Create Date: 2026-06-19
"""
from alembic import op
import sqlalchemy as sa

revision = '020_secrets_webhooks'
down_revision = '019_passkey_credentials'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    tables = inspector.get_table_names()

    if 'secret_vaults' not in tables:
        op.create_table(
            'secret_vaults',
            sa.Column('id', sa.Integer(), nullable=False),
            sa.Column('name', sa.String(length=200), nullable=False),
            sa.Column('slug', sa.String(length=220), nullable=False),
            sa.Column('description', sa.Text(), nullable=True),
            sa.Column('created_by', sa.Integer(), nullable=True),
            sa.Column('created_at', sa.DateTime(), nullable=True),
            sa.Column('updated_at', sa.DateTime(), nullable=True),
            sa.ForeignKeyConstraint(['created_by'], ['users.id']),
            sa.PrimaryKeyConstraint('id'),
            sa.UniqueConstraint('name'),
            sa.UniqueConstraint('slug')
        )
        op.create_index(
            op.f('ix_secret_vaults_created_by'),
            'secret_vaults',
            ['created_by'],
            unique=False
        )

    if 'secrets' not in tables:
        op.create_table(
            'secrets',
            sa.Column('id', sa.Integer(), nullable=False),
            sa.Column('vault_id', sa.Integer(), nullable=False),
            sa.Column('name', sa.String(length=200), nullable=False),
            sa.Column('encrypted_value', sa.Text(), nullable=False),
            sa.Column('description', sa.Text(), nullable=True),
            sa.Column('expires_at', sa.DateTime(), nullable=True),
            sa.Column('created_at', sa.DateTime(), nullable=True),
            sa.Column('updated_at', sa.DateTime(), nullable=True),
            sa.ForeignKeyConstraint(['vault_id'], ['secret_vaults.id'], ondelete='CASCADE'),
            sa.PrimaryKeyConstraint('id'),
            sa.UniqueConstraint('vault_id', 'name')
        )
        op.create_index(
            op.f('ix_secrets_vault_id'),
            'secrets',
            ['vault_id'],
            unique=False
        )

    if 'webhook_endpoints' not in tables:
        op.create_table(
            'webhook_endpoints',
            sa.Column('id', sa.Integer(), nullable=False),
            sa.Column('name', sa.String(length=200), nullable=False),
            sa.Column('slug', sa.String(length=220), nullable=False),
            sa.Column('secret', sa.String(length=500), nullable=False),
            sa.Column('forward_url', sa.String(length=500), nullable=True),
            sa.Column('filter_paths', sa.Text(), nullable=True),
            sa.Column('retry_count', sa.Integer(), nullable=True),
            sa.Column('is_active', sa.Boolean(), nullable=True),
            sa.Column('created_by', sa.Integer(), nullable=True),
            sa.Column('created_at', sa.DateTime(), nullable=True),
            sa.Column('updated_at', sa.DateTime(), nullable=True),
            sa.ForeignKeyConstraint(['created_by'], ['users.id']),
            sa.PrimaryKeyConstraint('id'),
            sa.UniqueConstraint('name'),
            sa.UniqueConstraint('slug')
        )
        op.create_index(
            op.f('ix_webhook_endpoints_created_by'),
            'webhook_endpoints',
            ['created_by'],
            unique=False
        )

    if 'webhook_deliveries' not in tables:
        op.create_table(
            'webhook_deliveries',
            sa.Column('id', sa.Integer(), nullable=False),
            sa.Column('endpoint_id', sa.Integer(), nullable=False),
            sa.Column('event_id', sa.String(length=300), nullable=False),
            sa.Column('payload', sa.Text(), nullable=True),
            sa.Column('headers', sa.Text(), nullable=True),
            sa.Column('signature_valid', sa.Boolean(), nullable=True),
            sa.Column('status', sa.String(length=50), nullable=True),
            sa.Column('response_status', sa.Integer(), nullable=True),
            sa.Column('response_body', sa.Text(), nullable=True),
            sa.Column('error_message', sa.Text(), nullable=True),
            sa.Column('received_at', sa.DateTime(), nullable=True),
            sa.Column('completed_at', sa.DateTime(), nullable=True),
            sa.ForeignKeyConstraint(['endpoint_id'], ['webhook_endpoints.id'], ondelete='CASCADE'),
            sa.PrimaryKeyConstraint('id'),
            sa.UniqueConstraint('endpoint_id', 'event_id')
        )
        op.create_index(
            op.f('ix_webhook_deliveries_endpoint_id'),
            'webhook_deliveries',
            ['endpoint_id'],
            unique=False
        )
        op.create_index(
            op.f('ix_webhook_deliveries_event_id'),
            'webhook_deliveries',
            ['event_id'],
            unique=False
        )


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    tables = inspector.get_table_names()

    if 'webhook_deliveries' in tables:
        op.drop_index(op.f('ix_webhook_deliveries_event_id'), table_name='webhook_deliveries')
        op.drop_index(op.f('ix_webhook_deliveries_endpoint_id'), table_name='webhook_deliveries')
        op.drop_table('webhook_deliveries')

    if 'webhook_endpoints' in tables:
        op.drop_index(op.f('ix_webhook_endpoints_created_by'), table_name='webhook_endpoints')
        op.drop_table('webhook_endpoints')

    if 'secrets' in tables:
        op.drop_index(op.f('ix_secrets_vault_id'), table_name='secrets')
        op.drop_table('secrets')

    if 'secret_vaults' in tables:
        op.drop_index(op.f('ix_secret_vaults_created_by'), table_name='secret_vaults')
        op.drop_table('secret_vaults')
