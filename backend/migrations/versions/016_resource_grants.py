"""Per-resource access grants (#33 — per-site ACL).

Adds:
- table resource_grants (one row per user-x-resource grant; resource_type +
  resource_id identify the shared resource, e.g. an application)

Idempotent: MigrationService._fix_missing_columns runs db.create_all() on boot
before Alembic (creating the table), so guard on the live schema like 009..015.

Revision ID: 016_resource_grants
Revises: 015_workspace_scope
Create Date: 2026-06-01
"""
from alembic import op
import sqlalchemy as sa

revision = '016_resource_grants'
down_revision = '015_workspace_scope'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'resource_grants' not in inspector.get_table_names():
        op.create_table(
            'resource_grants',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('user_id', sa.Integer(), sa.ForeignKey('users.id'), nullable=False, index=True),
            sa.Column('resource_type', sa.String(length=32), nullable=False),
            sa.Column('resource_id', sa.Integer(), nullable=False, index=True),
            sa.Column('role', sa.String(length=16)),
            sa.Column('granted_by', sa.Integer(), sa.ForeignKey('users.id')),
            sa.Column('created_at', sa.DateTime()),
            sa.UniqueConstraint('user_id', 'resource_type', 'resource_id', name='uq_resource_grant'),
        )


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'resource_grants' in inspector.get_table_names():
        op.drop_table('resource_grants')
