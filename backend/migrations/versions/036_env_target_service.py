"""Per-service targeting for env vars.

Adds:
- environment_variables.target_service
- shared_variables.target_service

NULL = all services. Lets a compose app scope a variable to one service in the
managed env overlay (docker-compose.serverkit.yml).

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 036_env_target_service
Revises: 035_app_ingress_plane
Create Date: 2026-06-24
"""
from alembic import op
import sqlalchemy as sa

revision = '036_env_target_service'
down_revision = '035_app_ingress_plane'
branch_labels = None
depends_on = None


def _add_col(inspector, table, col):
    if table in inspector.get_table_names():
        cols = {c['name'] for c in inspector.get_columns(table)}
        if col not in cols:
            op.add_column(table, sa.Column(col, sa.String(120), nullable=True))


def _drop_col(inspector, table, col):
    if table in inspector.get_table_names():
        cols = {c['name'] for c in inspector.get_columns(table)}
        if col in cols:
            op.drop_column(table, col)


def upgrade():
    inspector = sa.inspect(op.get_bind())
    _add_col(inspector, 'environment_variables', 'target_service')
    _add_col(inspector, 'shared_variables', 'target_service')


def downgrade():
    inspector = sa.inspect(op.get_bind())
    _drop_col(inspector, 'shared_variables', 'target_service')
    _drop_col(inspector, 'environment_variables', 'target_service')
