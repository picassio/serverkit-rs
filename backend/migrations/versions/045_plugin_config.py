"""Per-plugin config store (#49).

Adds:
- installed_plugins.config_json (saved config values; the manifest's
  config_schema describes the fields, plugins read them via
  plugins_sdk.config(slug))

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 045_plugin_config
Revises: 044_managed_databases
Create Date: 2026-07-02
"""
from alembic import op
import sqlalchemy as sa

revision = '045_plugin_config'
down_revision = '044_managed_databases'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'installed_plugins' not in set(inspector.get_table_names()):
        return
    cols = {c['name'] for c in inspector.get_columns('installed_plugins')}
    if 'config_json' not in cols:
        op.add_column('installed_plugins', sa.Column('config_json', sa.Text, nullable=True))


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'installed_plugins' not in set(inspector.get_table_names()):
        return
    cols = {c['name'] for c in inspector.get_columns('installed_plugins')}
    if 'config_json' in cols:
        op.drop_column('installed_plugins', 'config_json')
