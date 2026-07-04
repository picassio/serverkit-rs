"""Agent self-reported footprint.

Adds:
- servers.agent_install_dir (running binary's directory)
- servers.agent_config_dir  (directory of the loaded config file)

Both reported by the agent in system_info; target-aware UIs (File Manager
quick links) prefer them over installer-default conventions.

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 047_agent_footprint_dirs
Revises: 046_drop_legacy_marketplace
Create Date: 2026-07-02
"""
from alembic import op
import sqlalchemy as sa

revision = '047_agent_footprint_dirs'
down_revision = '046_drop_legacy_marketplace'
branch_labels = None
depends_on = None

_COLUMNS = ('agent_install_dir', 'agent_config_dir')


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'servers' not in set(inspector.get_table_names()):
        return
    cols = {c['name'] for c in inspector.get_columns('servers')}
    for name in _COLUMNS:
        if name not in cols:
            op.add_column('servers', sa.Column(name, sa.String(255), nullable=True))


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'servers' not in set(inspector.get_table_names()):
        return
    cols = {c['name'] for c in inspector.get_columns('servers')}
    for name in _COLUMNS:
        if name in cols:
            op.drop_column('servers', name)
