"""Add deployment jobs and app server targets.

Revision ID: 004_deployment_jobs
Revises: 003_workflows_automation
Create Date: 2026-04-29
"""

from alembic import op
import sqlalchemy as sa

revision = '004_deployment_jobs'
down_revision = '003_workflows_automation'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing_tables = inspector.get_table_names()

    if 'applications' in existing_tables:
        existing_cols = {c['name'] for c in inspector.get_columns('applications')}
        if 'server_id' not in existing_cols:
            with op.batch_alter_table('applications') as batch_op:
                batch_op.add_column(sa.Column('server_id', sa.String(36), nullable=True))
                batch_op.create_index('ix_applications_server_id', ['server_id'])
                batch_op.create_foreign_key(
                    'fk_applications_server_id_servers',
                    'servers',
                    ['server_id'],
                    ['id'],
                )

    if 'deployment_jobs' not in existing_tables:
        op.create_table(
            'deployment_jobs',
            sa.Column('id', sa.String(36), primary_key=True),
            sa.Column('kind', sa.String(50), nullable=False),
            sa.Column('status', sa.String(20), nullable=False, server_default='pending'),
            sa.Column('target_server_id', sa.String(36), sa.ForeignKey('servers.id'), nullable=True),
            sa.Column('app_id', sa.Integer(), sa.ForeignKey('applications.id'), nullable=True),
            sa.Column('requested_by', sa.Integer(), sa.ForeignKey('users.id'), nullable=True),
            sa.Column('trigger', sa.String(30), nullable=True, server_default='manual'),
            sa.Column('total_steps', sa.Integer(), nullable=True, server_default='0'),
            sa.Column('current_step', sa.Integer(), nullable=True, server_default='0'),
            sa.Column('current_step_name', sa.String(200), nullable=True),
            sa.Column('plan', sa.Text(), nullable=True, server_default='{}'),
            sa.Column('result', sa.Text(), nullable=True, server_default='{}'),
            sa.Column('error_message', sa.Text(), nullable=True),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('started_at', sa.DateTime(), nullable=True),
            sa.Column('completed_at', sa.DateTime(), nullable=True),
            sa.Column('updated_at', sa.DateTime(), server_default=sa.func.now()),
        )
        op.create_index('ix_deployment_jobs_kind', 'deployment_jobs', ['kind'])
        op.create_index('ix_deployment_jobs_status', 'deployment_jobs', ['status'])
        op.create_index('ix_deployment_jobs_target_server_id', 'deployment_jobs', ['target_server_id'])
        op.create_index('ix_deployment_jobs_app_id', 'deployment_jobs', ['app_id'])
        op.create_index('ix_deployment_jobs_created_at', 'deployment_jobs', ['created_at'])

    if 'deployment_job_logs' not in existing_tables:
        op.create_table(
            'deployment_job_logs',
            sa.Column('id', sa.BigInteger(), primary_key=True, autoincrement=True),
            sa.Column('job_id', sa.String(36), sa.ForeignKey('deployment_jobs.id'), nullable=False),
            sa.Column('step_index', sa.Integer(), nullable=True),
            sa.Column('level', sa.String(10), nullable=True, server_default='info'),
            sa.Column('message', sa.Text(), nullable=False),
            sa.Column('data', sa.Text(), nullable=True),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
        )
        op.create_index('ix_deployment_job_logs_job_id', 'deployment_job_logs', ['job_id'])
        op.create_index('ix_deployment_job_logs_created_at', 'deployment_job_logs', ['created_at'])


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing_tables = inspector.get_table_names()

    if 'deployment_job_logs' in existing_tables:
        op.drop_index('ix_deployment_job_logs_created_at', table_name='deployment_job_logs')
        op.drop_index('ix_deployment_job_logs_job_id', table_name='deployment_job_logs')
        op.drop_table('deployment_job_logs')

    if 'deployment_jobs' in existing_tables:
        op.drop_index('ix_deployment_jobs_created_at', table_name='deployment_jobs')
        op.drop_index('ix_deployment_jobs_app_id', table_name='deployment_jobs')
        op.drop_index('ix_deployment_jobs_target_server_id', table_name='deployment_jobs')
        op.drop_index('ix_deployment_jobs_status', table_name='deployment_jobs')
        op.drop_index('ix_deployment_jobs_kind', table_name='deployment_jobs')
        op.drop_table('deployment_jobs')

    if 'applications' in existing_tables:
        existing_cols = {c['name'] for c in inspector.get_columns('applications')}
        if 'server_id' in existing_cols:
            with op.batch_alter_table('applications') as batch_op:
                batch_op.drop_constraint('fk_applications_server_id_servers', type_='foreignkey')
                batch_op.drop_index('ix_applications_server_id')
                batch_op.drop_column('server_id')
