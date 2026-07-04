"""Add workflows and automation tables.

Revision ID: 003_workflows_automation
Revises: 002_permissions_invitations
Create Date: 2026-03-23
"""
from alembic import op
import sqlalchemy as sa
from datetime import datetime

revision = '003_workflows_automation'
down_revision = '002_permissions_invitations'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing_tables = inspector.get_table_names()

    # Create workflows table if missing (old version might have it but without automation fields)
    if 'workflows' not in existing_tables:
        op.create_table('workflows',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('name', sa.String(100), nullable=False),
            sa.Column('description', sa.Text(), nullable=True),
            sa.Column('nodes', sa.Text(), nullable=True),
            sa.Column('edges', sa.Text(), nullable=True),
            sa.Column('viewport', sa.Text(), nullable=True),
            sa.Column('is_active', sa.Boolean(), default=False),
            sa.Column('trigger_type', sa.String(50), default='manual'),
            sa.Column('trigger_config', sa.Text(), nullable=True),
            sa.Column('last_run_at', sa.DateTime(), nullable=True),
            sa.Column('last_status', sa.String(20), nullable=True),
            sa.Column('created_at', sa.DateTime(), default=datetime.utcnow),
            sa.Column('updated_at', sa.DateTime(), default=datetime.utcnow),
            sa.Column('user_id', sa.Integer(), sa.ForeignKey('users.id'), nullable=False)
        )
    else:
        # Add automation columns to existing workflows table if they don't exist
        existing_cols = {c['name'] for c in inspector.get_columns('workflows')}
        with op.batch_alter_table('workflows') as batch_op:
            if 'is_active' not in existing_cols:
                batch_op.add_column(sa.Column('is_active', sa.Boolean(), nullable=True, server_default='0'))
            if 'trigger_type' not in existing_cols:
                batch_op.add_column(sa.Column('trigger_type', sa.String(50), nullable=True, server_default='manual'))
            if 'trigger_config' not in existing_cols:
                batch_op.add_column(sa.Column('trigger_config', sa.Text(), nullable=True))
            if 'last_run_at' not in existing_cols:
                batch_op.add_column(sa.Column('last_run_at', sa.DateTime(), nullable=True))
            if 'last_status' not in existing_cols:
                batch_op.add_column(sa.Column('last_status', sa.String(20), nullable=True))

    # Create workflow_executions table
    if 'workflow_executions' not in existing_tables:
        op.create_table('workflow_executions',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('workflow_id', sa.Integer(), sa.ForeignKey('workflows.id'), nullable=False),
            sa.Column('status', sa.String(20), default='running'),
            sa.Column('trigger_type', sa.String(50), nullable=True),
            sa.Column('context', sa.Text(), nullable=True),
            sa.Column('results', sa.Text(), nullable=True),
            sa.Column('started_at', sa.DateTime(), default=datetime.utcnow),
            sa.Column('completed_at', sa.DateTime(), nullable=True)
        )

    # Create workflow_logs table
    if 'workflow_logs' not in existing_tables:
        op.create_table('workflow_logs',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('execution_id', sa.Integer(), sa.ForeignKey('workflow_executions.id'), nullable=False),
            sa.Column('level', sa.String(10), default='INFO'),
            sa.Column('message', sa.Text(), nullable=False),
            sa.Column('node_id', sa.String(100), nullable=True),
            sa.Column('timestamp', sa.DateTime(), default=datetime.utcnow)
        )


def downgrade():
    op.drop_table('workflow_logs')
    op.drop_table('workflow_executions')
    
    # We don't drop 'workflows' as it existed before, but we can drop the new columns
    with op.batch_alter_table('workflows') as batch_op:
        batch_op.drop_column('last_status')
        batch_op.drop_column('last_run_at')
        batch_op.drop_column('trigger_config')
        batch_op.drop_column('trigger_type')
        batch_op.drop_column('is_active')
