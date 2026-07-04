"""Drop the legacy marketplace catalog tables (#51).

The DB-seeded Extension/ExtensionInstall catalog was retired: nothing ever
populated `extensions` on a real panel (rows only came from a manual POST),
and install state lives on `installed_plugins`. Browse now merges the builtin
folder scan + the remote registry + InstalledPlugin state instead.

Idempotent: guarded on the live schema (the tables may already be absent on
fresh installs, since the models no longer exist for db.create_all()).

Revision ID: 046_drop_legacy_marketplace
Revises: 045_plugin_config
Create Date: 2026-07-02
"""
from alembic import op
import sqlalchemy as sa

revision = '046_drop_legacy_marketplace'
down_revision = '045_plugin_config'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    existing = set(sa.inspect(conn).get_table_names())
    # Child first (FK to extensions), then parent.
    if 'extension_installs' in existing:
        op.drop_table('extension_installs')
    if 'extensions' in existing:
        op.drop_table('extensions')


def downgrade():
    # The legacy catalog is gone for good; recreating empty tables would only
    # resurrect the confusion this migration removes. No-op.
    pass
