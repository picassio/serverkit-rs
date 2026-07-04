"""Link DNS zones to a canonical DNS provider connection.

Adds:
- dns_zones.dns_provider_config_id  (-> dns_provider_configs.id)

So the /dns Zones page and Dynamic DNS resolve Cloudflare credentials from the
same DNSProviderConfig store used by Settings -> Connections, instead of a second
token stashed inline in dns_zones.provider_config_json.

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema. The column is added without a DB-level
FK constraint (SQLite can't ALTER one in), matching how the boot column-sync adds
it; the ORM model carries the ForeignKey for relationship metadata.

Revision ID: 025_dns_provider_link
Revises: 024_email_providers
Create Date: 2026-06-22
"""
from alembic import op
import sqlalchemy as sa

revision = '025_dns_provider_link'
down_revision = '024_email_providers'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'dns_zones' not in set(inspector.get_table_names()):
        return
    cols = {c['name'] for c in inspector.get_columns('dns_zones')}
    if 'dns_provider_config_id' not in cols:
        op.add_column('dns_zones',
                      sa.Column('dns_provider_config_id', sa.Integer, nullable=True))


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'dns_zones' not in set(inspector.get_table_names()):
        return
    cols = {c['name'] for c in inspector.get_columns('dns_zones')}
    if 'dns_provider_config_id' in cols:
        op.drop_column('dns_zones', 'dns_provider_config_id')
