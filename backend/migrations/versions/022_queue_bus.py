"""Queue Bus foundation.

Adds:
- queue_groups
- queues
- queue_messages

Idempotent: MigrationService runs db.create_all() on boot before Alembic,
so guard on the live schema.

Revision ID: 022_queue_bus
Revises: 021_secrets_webhooks_workspace
Create Date: 2026-06-21
"""
from alembic import op
import sqlalchemy as sa

revision = '022_queue_bus'
down_revision = '021_secrets_webhooks_workspace'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    tables = set(inspector.get_table_names())

    if 'queue_groups' not in tables:
        op.create_table(
            'queue_groups',
            sa.Column('id', sa.String(36), nullable=False),
            sa.Column('slug', sa.String(128), nullable=False),
            sa.Column('name', sa.String(256), nullable=False),
            sa.Column('description', sa.Text, nullable=True),
            sa.Column('owner_type', sa.String(32), nullable=False, server_default='system'),
            sa.Column('owner_id', sa.String(128), nullable=True),
            sa.Column('config_json', sa.Text, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=False),
            sa.Column('updated_at', sa.DateTime, nullable=False),
            sa.PrimaryKeyConstraint('id'),
            sa.UniqueConstraint('slug'),
        )
        op.create_index('ix_queue_groups_slug', 'queue_groups', ['slug'])

    if 'queues' not in tables:
        op.create_table(
            'queues',
            sa.Column('id', sa.String(36), nullable=False),
            sa.Column('group_id', sa.String(36), nullable=False),
            sa.Column('slug', sa.String(128), nullable=False),
            sa.Column('name', sa.String(256), nullable=False),
            sa.Column('description', sa.Text, nullable=True),
            sa.Column('config_json', sa.Text, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=False),
            sa.Column('updated_at', sa.DateTime, nullable=False),
            sa.PrimaryKeyConstraint('id'),
            sa.ForeignKeyConstraint(['group_id'], ['queue_groups.id']),
            sa.UniqueConstraint('group_id', 'slug', name='uix_queue_group_slug'),
        )
        op.create_index('ix_queues_group_id', 'queues', ['group_id'])
        op.create_index('ix_queues_slug', 'queues', ['slug'])

    if 'queue_messages' not in tables:
        op.create_table(
            'queue_messages',
            sa.Column('id', sa.String(36), nullable=False),
            sa.Column('queue_id', sa.String(36), nullable=False),
            sa.Column('group_id', sa.String(36), nullable=False),
            sa.Column('status', sa.String(32), nullable=False),
            sa.Column('priority', sa.Integer, nullable=False),
            sa.Column('payload_json', sa.Text, nullable=False),
            sa.Column('result_json', sa.Text, nullable=True),
            sa.Column('error_message', sa.Text, nullable=True),
            sa.Column('attempts', sa.Integer, nullable=False),
            sa.Column('max_attempts', sa.Integer, nullable=False),
            sa.Column('visible_after', sa.DateTime, nullable=False),
            sa.Column('invisible_until', sa.DateTime, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=False),
            sa.Column('updated_at', sa.DateTime, nullable=False),
            sa.Column('completed_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.ForeignKeyConstraint(['queue_id'], ['queues.id']),
            sa.ForeignKeyConstraint(['group_id'], ['queue_groups.id']),
        )
        op.create_index('ix_queue_messages_queue_id', 'queue_messages', ['queue_id'])
        op.create_index('ix_queue_messages_group_id', 'queue_messages', ['group_id'])
        op.create_index('ix_queue_messages_status', 'queue_messages', ['status'])
        op.create_index('ix_queue_messages_priority', 'queue_messages', ['priority'])
        op.create_index('ix_queue_messages_visible_after', 'queue_messages', ['visible_after'])
        op.create_index('ix_queue_messages_invisible_until', 'queue_messages', ['invisible_until'])
        op.create_index('ix_queue_messages_created_at', 'queue_messages', ['created_at'])


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    tables = set(inspector.get_table_names())

    for table in ('queue_messages', 'queues', 'queue_groups'):
        if table in tables:
            op.drop_table(table)
