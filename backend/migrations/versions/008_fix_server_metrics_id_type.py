"""Fix server_metrics.id type for SQLite autoincrement.

The id column was originally declared BigInteger, which works fine on
PostgreSQL / MySQL but is broken on SQLite: SQLite only auto-generates
the rowid for a column declared exactly as ``INTEGER PRIMARY KEY``.
``BIGINT PRIMARY KEY`` is treated as a regular NOT NULL column with no
autoincrement, so every metrics insert fails with
``NOT NULL constraint failed: server_metrics.id`` and the dashboard
silently sits at zero — which is the symptom users on pre-fix DBs hit
once an agent paired and started sending heartbeats.

The model was updated to ``Integer`` (see app/models/server.py
ServerMetrics), but ``db.create_all()`` does not ALTER existing tables,
so existing SQLite installs keep the wrong column. This migration
detects the wrong type on SQLite and rebuilds the table from scratch.
Historical samples are disposable (the dashboard window is short and
new samples flow back in within a heartbeat interval).

Postgres / MySQL installs are left untouched — the BIGINT autoincrement
behaviour works correctly there, and rewriting the table on a busy
production DB just to get from BIGINT to INTEGER would be a regression.

Revision ID: 008_fix_server_metrics_id_type
Revises: 007_canonical_git_extension_route
Create Date: 2026-05-07
"""

from alembic import op
import sqlalchemy as sa


revision = '008_fix_server_metrics_id_type'
down_revision = '007_canonical_git_extension_route'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    if conn.dialect.name != 'sqlite':
        # Bug is SQLite-specific — see module docstring.
        return

    inspector = sa.inspect(conn)
    if 'server_metrics' not in inspector.get_table_names():
        # Fresh install path: create_all() will produce the table with
        # the correct schema; nothing to do here.
        return

    id_col = next(
        (c for c in inspector.get_columns('server_metrics') if c['name'] == 'id'),
        None,
    )
    if id_col is None:
        return

    # Already INTEGER (fresh installs and DBs hand-fixed pre-this-migration
    # land here) — no-op. Anything else means the bad BIGINT column.
    type_name = str(id_col['type']).upper()
    if type_name.startswith('INTEGER'):
        return

    op.drop_table('server_metrics')
    op.create_table(
        'server_metrics',
        sa.Column('id', sa.Integer, primary_key=True, autoincrement=True),
        sa.Column('server_id', sa.String(36), sa.ForeignKey('servers.id'), nullable=False, index=True),
        sa.Column('timestamp', sa.DateTime, index=True),
        sa.Column('cpu_percent', sa.Float),
        sa.Column('memory_percent', sa.Float),
        sa.Column('memory_used', sa.BigInteger),
        sa.Column('disk_percent', sa.Float),
        sa.Column('disk_used', sa.BigInteger),
        sa.Column('network_rx', sa.BigInteger),
        sa.Column('network_tx', sa.BigInteger),
        sa.Column('network_rx_rate', sa.Float),
        sa.Column('network_tx_rate', sa.Float),
        sa.Column('container_count', sa.Integer),
        sa.Column('container_running', sa.Integer),
        sa.Column('extra', sa.JSON),
    )
    op.create_index('ix_server_metrics_server_time', 'server_metrics', ['server_id', 'timestamp'])


def downgrade():
    # Reverting would reintroduce the BIGINT autoincrement bug on SQLite.
    # The fix is forward-only.
    pass
