"""Notification Bus foundation.

Adds:
- notifications
- notification_deliveries

Idempotent: MigrationService runs db.create_all() on boot before Alembic,
so guard on the live schema.

Revision ID: 023_notification_bus
Revises: 022_queue_bus
Create Date: 2026-06-21
"""
from alembic import op
import sqlalchemy as sa

revision = '023_notification_bus'
down_revision = '022_queue_bus'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    tables = set(inspector.get_table_names())

    if 'notifications' not in tables:
        op.create_table(
            'notifications',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('event_key', sa.String(120), nullable=False),
            sa.Column('category', sa.String(40), nullable=True, server_default='system'),
            sa.Column('severity', sa.String(20), nullable=True, server_default='info'),
            sa.Column('title', sa.String(255), nullable=False),
            sa.Column('body', sa.Text, nullable=True),
            sa.Column('data_json', sa.Text, nullable=True),
            sa.Column('audience', sa.String(255), nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
        )
        op.create_index('ix_notifications_event_key', 'notifications', ['event_key'])
        op.create_index('ix_notifications_category', 'notifications', ['category'])
        op.create_index('ix_notifications_severity', 'notifications', ['severity'])
        op.create_index('ix_notifications_created_at', 'notifications', ['created_at'])

    if 'notification_deliveries' not in tables:
        op.create_table(
            'notification_deliveries',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('notification_id', sa.Integer, nullable=False),
            sa.Column('recipient_user_id', sa.Integer, nullable=True),
            sa.Column('channel', sa.String(40), nullable=False),
            sa.Column('target', sa.String(512), nullable=True),
            sa.Column('status', sa.String(20), nullable=True, server_default='pending'),
            sa.Column('attempts', sa.Integer, nullable=True, server_default='0'),
            sa.Column('error', sa.Text, nullable=True),
            sa.Column('provider_message_id', sa.String(255), nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.Column('sent_at', sa.DateTime, nullable=True),
            sa.Column('read_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.ForeignKeyConstraint(['notification_id'], ['notifications.id']),
            sa.ForeignKeyConstraint(['recipient_user_id'], ['users.id']),
        )
        op.create_index('ix_notification_deliveries_notification_id', 'notification_deliveries', ['notification_id'])
        op.create_index('ix_notification_deliveries_recipient_user_id', 'notification_deliveries', ['recipient_user_id'])
        op.create_index('ix_notification_deliveries_channel', 'notification_deliveries', ['channel'])
        op.create_index('ix_notification_deliveries_status', 'notification_deliveries', ['status'])
        op.create_index('ix_notification_deliveries_created_at', 'notification_deliveries', ['created_at'])


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    tables = set(inspector.get_table_names())

    for table in ('notification_deliveries', 'notifications'):
        if table in tables:
            op.drop_table(table)
