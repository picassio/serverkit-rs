"""WebAuthn passkey credentials.

Adds:
- passkey_credentials table

Revision ID: 019_passkey_credentials
Revises: 018_image_scan_sbom
Create Date: 2026-06-19
"""
from alembic import op
import sqlalchemy as sa

revision = '019_passkey_credentials'
down_revision = '018_image_scan_sbom'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    tables = inspector.get_table_names()

    if 'passkey_credentials' not in tables:
        op.create_table(
            'passkey_credentials',
            sa.Column('id', sa.Integer(), nullable=False),
            sa.Column('user_id', sa.Integer(), nullable=False),
            sa.Column('credential_id', sa.String(length=500), nullable=False),
            sa.Column('public_key', sa.Text(), nullable=False),
            sa.Column('sign_count', sa.Integer(), nullable=True),
            sa.Column('transports', sa.Text(), nullable=True),
            sa.Column('device_name', sa.String(length=200), nullable=True),
            sa.Column('is_active', sa.Boolean(), nullable=True),
            sa.Column('created_at', sa.DateTime(), nullable=True),
            sa.Column('last_used_at', sa.DateTime(), nullable=True),
            sa.ForeignKeyConstraint(['user_id'], ['users.id']),
            sa.PrimaryKeyConstraint('id')
        )
        op.create_index(
            op.f('ix_passkey_credentials_user_id'),
            'passkey_credentials',
            ['user_id'],
            unique=False
        )
        op.create_index(
            op.f('ix_passkey_credentials_credential_id'),
            'passkey_credentials',
            ['credential_id'],
            unique=True
        )


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    tables = inspector.get_table_names()

    if 'passkey_credentials' in tables:
        op.drop_index(op.f('ix_passkey_credentials_credential_id'), table_name='passkey_credentials')
        op.drop_index(op.f('ix_passkey_credentials_user_id'), table_name='passkey_credentials')
        op.drop_table('passkey_credentials')
