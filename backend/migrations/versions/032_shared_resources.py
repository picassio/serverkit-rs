"""Polymorphic shared resources (facade layer).

Adds:
- resource_tags
- shared_variable_groups, shared_variables, shared_variable_group_attachments

Tags and shared variable groups attach to any resource via (resource_type,
resource_id). Existing per-resource env var tables are untouched (facade first).

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 032_shared_resources
Revises: 031_projects_environments
Create Date: 2026-06-23
"""
from alembic import op
import sqlalchemy as sa

revision = '032_shared_resources'
down_revision = '031_projects_environments'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())

    if 'resource_tags' not in existing:
        op.create_table(
            'resource_tags',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('resource_type', sa.String(50), nullable=False),
            sa.Column('resource_id', sa.String(255), nullable=False),
            sa.Column('tag', sa.String(100), nullable=False),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.UniqueConstraint('resource_type', 'resource_id', 'tag', name='uq_resource_tag'),
        )
        op.create_index('ix_resource_tags_resource', 'resource_tags', ['resource_type', 'resource_id'])

    if 'shared_variable_groups' not in existing:
        op.create_table(
            'shared_variable_groups',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('scope_type', sa.String(50), nullable=False),
            sa.Column('scope_id', sa.String(255), nullable=False),
            sa.Column('name', sa.String(255), nullable=False),
            sa.Column('description', sa.String(500), nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.Column('updated_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
        )
        op.create_index('ix_shared_variable_groups_scope', 'shared_variable_groups', ['scope_type', 'scope_id'])

    if 'shared_variables' not in existing:
        op.create_table(
            'shared_variables',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('group_id', sa.Integer, nullable=False),
            sa.Column('key', sa.String(255), nullable=False),
            sa.Column('encrypted_value', sa.Text, nullable=False),
            sa.Column('is_secret', sa.Boolean, nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.ForeignKeyConstraint(['group_id'], ['shared_variable_groups.id']),
            sa.UniqueConstraint('group_id', 'key', name='uq_shared_variable_group_key'),
        )

    if 'shared_variable_group_attachments' not in existing:
        op.create_table(
            'shared_variable_group_attachments',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('group_id', sa.Integer, nullable=False),
            sa.Column('resource_type', sa.String(50), nullable=False),
            sa.Column('resource_id', sa.String(255), nullable=False),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.ForeignKeyConstraint(['group_id'], ['shared_variable_groups.id']),
            sa.UniqueConstraint('group_id', 'resource_type', 'resource_id', name='uq_shared_variable_attachment'),
        )
        op.create_index(
            'ix_shared_variable_group_attachments_resource',
            'shared_variable_group_attachments', ['resource_type', 'resource_id'],
        )


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())
    for tbl in ('shared_variable_group_attachments', 'shared_variables', 'shared_variable_groups', 'resource_tags'):
        if tbl in existing:
            op.drop_table(tbl)
