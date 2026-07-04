"""Per-server managed proxy stack.

Adds:
- proxy_stacks (one row per server; opt-in Traefik/Caddy compose stack, host
  Nginx stays the default)

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 034_proxy_stacks
Revises: 033_application_previews
Create Date: 2026-06-23
"""
from alembic import op
import sqlalchemy as sa

revision = '034_proxy_stacks'
down_revision = '033_application_previews'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())

    if 'proxy_stacks' not in existing:
        op.create_table(
            'proxy_stacks',
            sa.Column('id', sa.String(36), nullable=False),
            sa.Column('server_id', sa.String(36), nullable=False),
            sa.Column('proxy_type', sa.String(20), nullable=False, server_default='nginx'),
            sa.Column('status', sa.String(20), nullable=False, server_default='unknown'),
            sa.Column('compose_path', sa.String(512), nullable=True),
            sa.Column('networks', sa.Text, nullable=True),
            sa.Column('custom_snippet', sa.Text, nullable=True),
            sa.Column('last_regenerated_at', sa.DateTime, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.Column('updated_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.ForeignKeyConstraint(['server_id'], ['servers.id']),
            sa.UniqueConstraint('server_id', name='uq_proxy_stack_server'),
        )
        op.create_index('ix_proxy_stacks_server_id', 'proxy_stacks', ['server_id'])


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())
    if 'proxy_stacks' in existing:
        op.drop_table('proxy_stacks')
