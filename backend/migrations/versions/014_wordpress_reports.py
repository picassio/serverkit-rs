"""Monthly client reports (#33 — agency-scale reports slice).

Adds:
- table wordpress_reports (one persisted report per site per calendar month,
  FK site_id -> wordpress_sites.id)

Idempotent: MigrationService._fix_missing_columns runs db.create_all() on boot
before Alembic, so the table may already exist — guard on the live schema
exactly like 009/010/011/012/013.

Revision ID: 014_wordpress_reports
Revises: 013_ai_assistant
Create Date: 2026-06-01
"""
from alembic import op
import sqlalchemy as sa

revision = '014_wordpress_reports'
down_revision = '013_ai_assistant'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)

    if 'wordpress_reports' not in inspector.get_table_names():
        op.create_table(
            'wordpress_reports',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('site_id', sa.Integer(), sa.ForeignKey('wordpress_sites.id'), nullable=False, index=True),
            sa.Column('period_label', sa.String(length=7), nullable=False),
            sa.Column('period_start', sa.DateTime(), nullable=False),
            sa.Column('period_end', sa.DateTime(), nullable=False),
            sa.Column('data', sa.Text()),
            sa.Column('generated_at', sa.DateTime()),
        )


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'wordpress_reports' in inspector.get_table_names():
        op.drop_table('wordpress_reports')
