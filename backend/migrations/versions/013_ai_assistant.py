"""AI assistant core primitive (powered by Prompture).

Adds the three tables backing the in-panel assistant:
- ai_conversations (one chat thread per user; holds the Prompture export blob)
- ai_messages (denormalized per-turn rows for fast transcript rendering)
- ai_pending_actions (durable record of guarded write-tool confirmations)

Idempotent: MigrationService._fix_missing_columns runs db.create_all() on boot
before Alembic, so these tables may already exist — guard on the live schema
exactly like 009/010/011.

Revision ID: 013_ai_assistant
Revises: 012_wordpress_update_runs
Create Date: 2026-06-01
"""
from alembic import op
import sqlalchemy as sa

revision = '013_ai_assistant'
down_revision = '012_wordpress_update_runs'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())

    if 'ai_conversations' not in existing:
        op.create_table(
            'ai_conversations',
            sa.Column('id', sa.String(length=64), primary_key=True),
            sa.Column('user_id', sa.Integer(), sa.ForeignKey('users.id'), nullable=False, index=True),
            sa.Column('title', sa.String(length=256)),
            sa.Column('mode', sa.String(length=16)),
            sa.Column('model_name', sa.String(length=128)),
            sa.Column('export_json', sa.Text()),
            sa.Column('last_page', sa.String(length=256)),
            sa.Column('created_at', sa.DateTime()),
            sa.Column('updated_at', sa.DateTime()),
        )

    if 'ai_messages' not in existing:
        op.create_table(
            'ai_messages',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('conversation_id', sa.String(length=64),
                      sa.ForeignKey('ai_conversations.id', ondelete='CASCADE'), nullable=False, index=True),
            sa.Column('role', sa.String(length=16), nullable=False),
            sa.Column('content', sa.Text()),
            sa.Column('tool_calls_json', sa.Text()),
            sa.Column('usage_json', sa.Text()),
            sa.Column('created_at', sa.DateTime()),
        )

    if 'ai_pending_actions' not in existing:
        op.create_table(
            'ai_pending_actions',
            sa.Column('id', sa.String(length=64), primary_key=True),
            sa.Column('conversation_id', sa.String(length=64),
                      sa.ForeignKey('ai_conversations.id', ondelete='CASCADE'), nullable=False, index=True),
            sa.Column('user_id', sa.Integer(), sa.ForeignKey('users.id'), nullable=False, index=True),
            sa.Column('tool_name', sa.String(length=128), nullable=False),
            sa.Column('plugin_slug', sa.String(length=128)),
            sa.Column('params_json', sa.Text()),
            sa.Column('summary', sa.Text()),
            sa.Column('status', sa.String(length=16)),
            sa.Column('result_json', sa.Text()),
            sa.Column('created_at', sa.DateTime()),
            sa.Column('expires_at', sa.DateTime()),
        )


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())
    for table in ('ai_pending_actions', 'ai_messages', 'ai_conversations'):
        if table in existing:
            op.drop_table(table)
