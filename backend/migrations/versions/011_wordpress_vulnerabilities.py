"""Per-site WordPress vulnerability findings (#28).

Adds:
- table wordpress_vulnerabilities (findings cross-referenced against the
  WPVulnerability feed, FK site_id -> wordpress_sites.id)
- wordpress_sites.last_vuln_scan_at (nullable DateTime) so a "scanned & clean"
  site is distinguishable from a never-scanned one.

Idempotent: MigrationService._fix_missing_columns runs db.create_all() (creating
the new table) and ALTER-adds the new column on boot before Alembic runs, so
guard on the live schema — exactly like 009/010.

Revision ID: 011_wordpress_vulnerabilities
Revises: 010_wordpress_status_monitoring
Create Date: 2026-06-01
"""
from alembic import op
import sqlalchemy as sa

revision = '011_wordpress_vulnerabilities'
down_revision = '010_wordpress_status_monitoring'
branch_labels = None
depends_on = None


def _has_col(inspector, table, col):
    if table not in inspector.get_table_names():
        return False
    return col in {c['name'] for c in inspector.get_columns(table)}


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)

    if 'wordpress_vulnerabilities' not in inspector.get_table_names():
        op.create_table(
            'wordpress_vulnerabilities',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('site_id', sa.Integer(), sa.ForeignKey('wordpress_sites.id'), nullable=False, index=True),
            sa.Column('source', sa.String(length=20), nullable=False),
            sa.Column('slug', sa.String(length=200)),
            sa.Column('name', sa.String(length=255)),
            sa.Column('installed_version', sa.String(length=50)),
            sa.Column('advisory_id', sa.String(length=100)),
            sa.Column('title', sa.Text()),
            sa.Column('severity', sa.String(length=20)),
            sa.Column('cvss_score', sa.String(length=10)),
            sa.Column('fixed_in', sa.String(length=50)),
            sa.Column('reference_url', sa.String(length=500)),
            sa.Column('detected_at', sa.DateTime()),
        )

    if 'wordpress_sites' in inspector.get_table_names() and \
            not _has_col(inspector, 'wordpress_sites', 'last_vuln_scan_at'):
        with op.batch_alter_table('wordpress_sites') as batch_op:
            batch_op.add_column(sa.Column('last_vuln_scan_at', sa.DateTime(), nullable=True))


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if _has_col(inspector, 'wordpress_sites', 'last_vuln_scan_at'):
        with op.batch_alter_table('wordpress_sites') as batch_op:
            batch_op.drop_column('last_vuln_scan_at')
    if 'wordpress_vulnerabilities' in inspector.get_table_names():
        op.drop_table('wordpress_vulnerabilities')
