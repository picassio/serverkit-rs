"""WordPress global plugin library.

Adds:
- wordpress_custom_plugins (operator-owned plugins registered in the library)
- wordpress_site_plugins (per-site installations of library plugins)

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 041_wordpress_plugin_library
Revises: 040_site_base_domains
Create Date: 2026-07-01
"""
from alembic import op
import sqlalchemy as sa

revision = '041_wordpress_plugin_library'
down_revision = '040_site_base_domains'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())

    if 'wordpress_custom_plugins' not in existing:
        op.create_table(
            'wordpress_custom_plugins',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('slug', sa.String(200), nullable=False),
            sa.Column('name', sa.String(255), nullable=True),
            sa.Column('description', sa.Text, nullable=True),
            sa.Column('version', sa.String(50), nullable=True),
            sa.Column('author', sa.String(255), nullable=True),
            sa.Column('source_type', sa.String(20), nullable=False, server_default='github'),
            sa.Column('source_url', sa.String(500), nullable=False),
            sa.Column('branch', sa.String(100), nullable=True, server_default='main'),
            sa.Column('connection_id', sa.Integer, nullable=True),
            sa.Column('is_active', sa.Boolean, nullable=False, server_default=sa.true()),
            sa.Column('last_synced_at', sa.DateTime, nullable=True),
            sa.Column('sync_error', sa.Text, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.Column('updated_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.UniqueConstraint('slug', name='uq_wp_custom_plugin_slug'),
        )
        op.create_index('ix_wordpress_custom_plugins_slug',
                        'wordpress_custom_plugins', ['slug'])

    if 'wordpress_site_plugins' not in existing:
        op.create_table(
            'wordpress_site_plugins',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('wordpress_site_id', sa.Integer, nullable=False),
            sa.Column('custom_plugin_id', sa.Integer, nullable=False),
            sa.Column('installed_version', sa.String(50), nullable=True),
            sa.Column('status', sa.String(20), nullable=True, server_default='not_installed'),
            sa.Column('updated_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.ForeignKeyConstraint(['wordpress_site_id'], ['wordpress_sites.id'],
                                    ondelete='CASCADE'),
            sa.ForeignKeyConstraint(['custom_plugin_id'], ['wordpress_custom_plugins.id'],
                                    ondelete='CASCADE'),
            sa.UniqueConstraint('wordpress_site_id', 'custom_plugin_id',
                                name='uq_site_custom_plugin'),
        )
        op.create_index('ix_wordpress_site_plugins_site',
                        'wordpress_site_plugins', ['wordpress_site_id'])
        op.create_index('ix_wordpress_site_plugins_plugin',
                        'wordpress_site_plugins', ['custom_plugin_id'])


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())
    if 'wordpress_site_plugins' in existing:
        op.drop_table('wordpress_site_plugins')
    if 'wordpress_custom_plugins' in existing:
        op.drop_table('wordpress_custom_plugins')
