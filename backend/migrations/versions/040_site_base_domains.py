"""Registry of base domains managed sites can be published under.

Adds:
- site_base_domains (one row per base domain, e.g. example.com / toto.com; each
  with its own DNS provider + wildcard-cert state; exactly one is the default)

Rows are seeded lazily by SiteBaseDomainService.ensure_seeded from the legacy
``sites_base_domain`` setting the first time a second domain is registered, so
this migration only creates the (empty) table.

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 040_site_base_domains
Revises: 039_deployment_job_links
Create Date: 2026-07-01
"""
from alembic import op
import sqlalchemy as sa

revision = '040_site_base_domains'
down_revision = '039_deployment_job_links'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())

    if 'site_base_domains' not in existing:
        op.create_table(
            'site_base_domains',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('domain', sa.String(253), nullable=False),
            sa.Column('is_default', sa.Boolean, nullable=False, server_default=sa.false()),
            sa.Column('dns_mode', sa.String(20), nullable=False, server_default='wildcard'),
            sa.Column('https_enabled', sa.Boolean, nullable=False, server_default=sa.false()),
            sa.Column('dns_provider_config_id', sa.Integer, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.Column('updated_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.ForeignKeyConstraint(['dns_provider_config_id'], ['dns_provider_configs.id'],
                                    ondelete='SET NULL'),
            sa.UniqueConstraint('domain', name='uq_site_base_domain'),
        )


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())
    if 'site_base_domains' in existing:
        op.drop_table('site_base_domains')
