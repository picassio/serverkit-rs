"""Add permissions column to users and create invitations table.

Revision ID: 002_permissions_invitations
Revises: 001_baseline
Create Date: 2026-03-04
"""
from alembic import op
import sqlalchemy as sa

revision = '002_permissions_invitations'
down_revision = '001_baseline'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing_tables = inspector.get_table_names()

    # Add permissions column to users if missing
    if 'users' in existing_tables:
        existing_cols = {c['name'] for c in inspector.get_columns('users')}
        if 'permissions' not in existing_cols:
            with op.batch_alter_table('users') as batch_op:
                batch_op.add_column(sa.Column('permissions', sa.Text(), nullable=True))

    # Create invitations table
    if 'invitations' not in existing_tables:
        op.create_table('invitations',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('email', sa.String(255), nullable=True),
            sa.Column('token', sa.String(64), unique=True, nullable=False, index=True),
            sa.Column('role', sa.String(20), nullable=False, server_default='developer'),
            sa.Column('permissions', sa.Text(), nullable=True),
            sa.Column('invited_by', sa.Integer(), sa.ForeignKey('users.id'), nullable=False),
            sa.Column('expires_at', sa.DateTime(), nullable=True),
            sa.Column('accepted_at', sa.DateTime(), nullable=True),
            sa.Column('accepted_by', sa.Integer(), sa.ForeignKey('users.id'), nullable=True),
            sa.Column('status', sa.String(20), nullable=False, server_default='pending', index=True),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
        )


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing_tables = inspector.get_table_names()

    if 'invitations' in existing_tables:
        op.drop_table('invitations')

    if 'users' in existing_tables:
        existing_cols = {c['name'] for c in inspector.get_columns('users')}
        if 'permissions' in existing_cols:
            with op.batch_alter_table('users') as batch_op:
                batch_op.drop_column('permissions')
