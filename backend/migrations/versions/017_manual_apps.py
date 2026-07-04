"""Manual and upload-based application support.

Adds:
- applications.source (github | template | manual | upload)
- applications.compose_file
- applications.systemd_unit
- applications.managed_by (docker_compose | systemd)
- applications.version
- applications.upload_path

Idempotent: MigrationService._fix_missing_columns runs db.create_all() on boot
before Alembic, so guard on the live schema like previous migrations.

Revision ID: 017_manual_apps
Revises: 016_resource_grants
Create Date: 2026-06-19
"""
from alembic import op
import sqlalchemy as sa

revision = '017_manual_apps'
down_revision = '016_resource_grants'
branch_labels = None
depends_on = None


def _has_col(inspector, table, col):
    if table not in inspector.get_table_names():
        return False
    return col in {c['name'] for c in inspector.get_columns(table)}


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)

    if 'applications' not in inspector.get_table_names():
        return

    cols = [
        ('source', sa.String(length=20)),
        ('compose_file', sa.String(length=200)),
        ('systemd_unit', sa.String(length=100)),
        ('managed_by', sa.String(length=20)),
        ('version', sa.Integer()),
        ('upload_path', sa.String(length=500)),
    ]

    with op.batch_alter_table('applications') as batch_op:
        for name, col_type in cols:
            if not _has_col(inspector, 'applications', name):
                kwargs = {}
                if name == 'source':
                    kwargs['nullable'] = False
                    kwargs['server_default'] = 'github'
                if name == 'version':
                    kwargs['nullable'] = False
                    kwargs['server_default'] = '0'
                batch_op.add_column(sa.Column(name, col_type, **kwargs))

    # Backfill existing rows that have no source set
    conn.execute(sa.text("UPDATE applications SET source = 'github' WHERE source IS NULL OR source = ''"))


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)

    if 'applications' not in inspector.get_table_names():
        return

    with op.batch_alter_table('applications') as batch_op:
        for name in ('upload_path', 'version', 'managed_by', 'systemd_unit', 'compose_file', 'source'):
            if _has_col(inspector, 'applications', name):
                batch_op.drop_column(name)
