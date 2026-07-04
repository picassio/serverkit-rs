"""Image vulnerability scans and SBOM artifacts.

Adds:
- image_vulnerability_scans table
- sbom_artifacts table

Revision ID: 018_image_scan_sbom
Revises: 017_manual_apps
Create Date: 2026-06-19
"""
from alembic import op
import sqlalchemy as sa

revision = '018_image_scan_sbom'
down_revision = '017_manual_apps'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    tables = inspector.get_table_names()

    if 'image_vulnerability_scans' not in tables:
        op.create_table(
            'image_vulnerability_scans',
            sa.Column('id', sa.Integer(), nullable=False),
            sa.Column('application_id', sa.Integer(), nullable=False),
            sa.Column('image_ref', sa.String(length=500), nullable=False),
            sa.Column('scanner', sa.String(length=50), nullable=True),
            sa.Column('scanner_version', sa.String(length=50), nullable=True),
            sa.Column('status', sa.String(length=20), nullable=True),
            sa.Column('severity_counts', sa.Text(), nullable=True),
            sa.Column('findings', sa.Text(), nullable=True),
            sa.Column('error_message', sa.Text(), nullable=True),
            sa.Column('started_at', sa.DateTime(), nullable=True),
            sa.Column('completed_at', sa.DateTime(), nullable=True),
            sa.ForeignKeyConstraint(['application_id'], ['applications.id']),
            sa.PrimaryKeyConstraint('id')
        )
        op.create_index(
            op.f('ix_image_vulnerability_scans_application_id'),
            'image_vulnerability_scans',
            ['application_id'],
            unique=False
        )

    if 'sbom_artifacts' not in tables:
        op.create_table(
            'sbom_artifacts',
            sa.Column('id', sa.Integer(), nullable=False),
            sa.Column('application_id', sa.Integer(), nullable=False),
            sa.Column('image_ref', sa.String(length=500), nullable=False),
            sa.Column('generator', sa.String(length=50), nullable=True),
            sa.Column('generator_version', sa.String(length=50), nullable=True),
            sa.Column('format', sa.String(length=20), nullable=True),
            sa.Column('sbom_json', sa.Text(), nullable=True),
            sa.Column('created_at', sa.DateTime(), nullable=True),
            sa.ForeignKeyConstraint(['application_id'], ['applications.id']),
            sa.PrimaryKeyConstraint('id')
        )
        op.create_index(
            op.f('ix_sbom_artifacts_application_id'),
            'sbom_artifacts',
            ['application_id'],
            unique=False
        )


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    tables = inspector.get_table_names()

    if 'sbom_artifacts' in tables:
        op.drop_index(op.f('ix_sbom_artifacts_application_id'), table_name='sbom_artifacts')
        op.drop_table('sbom_artifacts')

    if 'image_vulnerability_scans' in tables:
        op.drop_index(op.f('ix_image_vulnerability_scans_application_id'), table_name='image_vulnerability_scans')
        op.drop_table('image_vulnerability_scans')
