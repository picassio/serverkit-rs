"""Safe-update manager: run history + per-site schedule (#29).

Adds:
- table wordpress_update_runs (one row per safe-update run, FK site_id -> wordpress_sites.id)
- wordpress_sites.auto_update_schedule (cron string, null = off)
- wordpress_sites.auto_update_exclude  (JSON list of slugs to skip)

Idempotent: MigrationService._fix_missing_columns runs db.create_all() (creating
the table) and ALTER-adds the columns on boot before Alembic, so guard on the
live schema — like 009/010/011.

Revision ID: 012_wordpress_update_runs
Revises: 011_wordpress_vulnerabilities
Create Date: 2026-06-01
"""
from alembic import op
import sqlalchemy as sa

revision = '012_wordpress_update_runs'
down_revision = '011_wordpress_vulnerabilities'
branch_labels = None
depends_on = None


def _has_col(inspector, table, col):
    if table not in inspector.get_table_names():
        return False
    return col in {c['name'] for c in inspector.get_columns(table)}


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)

    if 'wordpress_update_runs' not in inspector.get_table_names():
        op.create_table(
            'wordpress_update_runs',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('site_id', sa.Integer(), sa.ForeignKey('wordpress_sites.id'), nullable=False, index=True),
            sa.Column('status', sa.String(length=20)),
            sa.Column('trigger', sa.String(length=20)),
            sa.Column('details', sa.Text()),
            sa.Column('error', sa.Text()),
            sa.Column('started_at', sa.DateTime()),
            sa.Column('finished_at', sa.DateTime()),
        )

    if 'wordpress_sites' in inspector.get_table_names():
        with op.batch_alter_table('wordpress_sites') as batch_op:
            if not _has_col(inspector, 'wordpress_sites', 'auto_update_schedule'):
                batch_op.add_column(sa.Column('auto_update_schedule', sa.String(length=100), nullable=True))
            if not _has_col(inspector, 'wordpress_sites', 'auto_update_exclude'):
                batch_op.add_column(sa.Column('auto_update_exclude', sa.Text(), nullable=True))


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    with op.batch_alter_table('wordpress_sites') as batch_op:
        if _has_col(inspector, 'wordpress_sites', 'auto_update_exclude'):
            batch_op.drop_column('auto_update_exclude')
        if _has_col(inspector, 'wordpress_sites', 'auto_update_schedule'):
            batch_op.drop_column('auto_update_schedule')
    if 'wordpress_update_runs' in inspector.get_table_names():
        op.drop_table('wordpress_update_runs')
