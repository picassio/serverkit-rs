"""Cache capability snapshot per server.

Adds JSON columns to ``servers`` so the panel can render the Overview
tab when the agent is offline. Written by agent_registry.update_
capabilities every time an agent reports state.

Revision ID: 005_server_capabilities_cache
Revises: 004_deployment_jobs
Create Date: 2026-05-01
"""

from alembic import op
import sqlalchemy as sa

revision = '005_server_capabilities_cache'
down_revision = '004_deployment_jobs'
branch_labels = None
depends_on = None


_NEW_COLUMNS = [
    ('cached_capabilities', sa.JSON()),
    ('cached_runtimes', sa.JSON()),
    ('cached_runtime_managers', sa.JSON()),
    ('cached_allowed_paths', sa.JSON()),
    ('cached_sudo', sa.String(20)),
    ('cached_systemd_json', sa.Boolean()),
    ('capabilities_at', sa.DateTime()),
]


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'servers' not in inspector.get_table_names():
        return
    existing = {c['name'] for c in inspector.get_columns('servers')}
    with op.batch_alter_table('servers') as batch_op:
        for name, type_ in _NEW_COLUMNS:
            if name not in existing:
                batch_op.add_column(sa.Column(name, type_, nullable=True))


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'servers' not in inspector.get_table_names():
        return
    existing = {c['name'] for c in inspector.get_columns('servers')}
    with op.batch_alter_table('servers') as batch_op:
        for name, _ in _NEW_COLUMNS:
            if name in existing:
                batch_op.drop_column(name)
