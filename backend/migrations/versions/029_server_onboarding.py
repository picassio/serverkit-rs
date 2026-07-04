"""Server onboarding state machine.

Adds:
- server_onboarding_logs (one row per onboarding step transition)
- servers.onboarding_state / onboarding_progress / onboarding_updated_at

Backs the observable server onboarding lifecycle
(pending -> validating -> installing_prerequisites -> installing_docker ->
pairing_agent -> ready / failed) surfaced in the server detail wizard.

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 029_server_onboarding
Revises: 028_backup_policies
Create Date: 2026-06-23
"""
from alembic import op
import sqlalchemy as sa

revision = '029_server_onboarding'
down_revision = '028_backup_policies'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())

    if 'server_onboarding_logs' not in existing:
        op.create_table(
            'server_onboarding_logs',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('server_id', sa.String(36), nullable=False),
            sa.Column('state', sa.String(40), nullable=False),
            sa.Column('status', sa.String(20), nullable=False),
            sa.Column('message', sa.Text, nullable=True),
            sa.Column('detail_json', sa.Text, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
        )
        op.create_index('ix_server_onboarding_logs_server_id', 'server_onboarding_logs', ['server_id'])
        op.create_index('ix_server_onboarding_logs_created_at', 'server_onboarding_logs', ['created_at'])

    if 'servers' in existing:
        cols = {c['name'] for c in inspector.get_columns('servers')}
        if 'onboarding_state' not in cols:
            op.add_column('servers', sa.Column('onboarding_state', sa.String(40), nullable=True, server_default='pending'))
        if 'onboarding_progress' not in cols:
            op.add_column('servers', sa.Column('onboarding_progress', sa.Text, nullable=True))
        if 'onboarding_updated_at' not in cols:
            op.add_column('servers', sa.Column('onboarding_updated_at', sa.DateTime, nullable=True))


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())
    if 'servers' in existing:
        cols = {c['name'] for c in inspector.get_columns('servers')}
        for col in ('onboarding_updated_at', 'onboarding_progress', 'onboarding_state'):
            if col in cols:
                op.drop_column('servers', col)
    if 'server_onboarding_logs' in existing:
        op.drop_table('server_onboarding_logs')
