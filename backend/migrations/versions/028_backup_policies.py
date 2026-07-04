"""Backup protection policies + runs.

Adds:
- backup_policies
- backup_runs

Backs the WordPress/Services "Protection" panel: one policy per target, with the
cron schedule mirrored into a ScheduledJob, and one backup_runs row per execution
(the history view's source of truth).

Idempotent: MigrationService runs _fix_missing_columns() + db.create_all() on boot
before Alembic, so guard on the live schema.

Revision ID: 028_backup_policies
Revises: 027_dns_change_log
Create Date: 2026-06-22
"""
from alembic import op
import sqlalchemy as sa

revision = '028_backup_policies'
down_revision = '027_dns_change_log'
branch_labels = None
depends_on = None


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())

    if 'backup_policies' not in existing:
        op.create_table(
            'backup_policies',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('target_type', sa.String(40), nullable=False),
            sa.Column('target_id', sa.Integer, nullable=False),
            sa.Column('enabled', sa.Boolean, nullable=False, server_default=sa.false()),
            sa.Column('schedule_cron', sa.String(120), nullable=False, server_default='0 2 * * *'),
            sa.Column('retention_count', sa.Integer, nullable=False, server_default='14'),
            sa.Column('retention_days', sa.Integer, nullable=False, server_default='30'),
            sa.Column('full_every_n_days', sa.Integer, nullable=False, server_default='7'),
            sa.Column('compression', sa.String(20), nullable=False, server_default='balanced'),
            sa.Column('remote_copy', sa.Boolean, nullable=False, server_default=sa.false()),
            sa.Column('pre_backup_hook', sa.Text, nullable=True),
            sa.Column('post_backup_hook', sa.Text, nullable=True),
            sa.Column('last_run_at', sa.DateTime, nullable=True),
            sa.Column('last_status', sa.String(20), nullable=True),
            sa.Column('last_size', sa.BigInteger, nullable=True),
            sa.Column('last_cost_local', sa.Numeric(10, 4), nullable=True),
            sa.Column('last_cost_remote', sa.Numeric(10, 4), nullable=True),
            sa.Column('last_job_id', sa.String(36), nullable=True),
            sa.Column('created_at', sa.DateTime, nullable=True),
            sa.Column('updated_at', sa.DateTime, nullable=True),
            sa.PrimaryKeyConstraint('id'),
            sa.UniqueConstraint('target_type', 'target_id', name='uq_backup_policy_target'),
        )
        op.create_index('ix_backup_policies_target_type', 'backup_policies', ['target_type'])
        op.create_index('ix_backup_policies_target_id', 'backup_policies', ['target_id'])

    if 'backup_runs' not in existing:
        op.create_table(
            'backup_runs',
            sa.Column('id', sa.Integer, nullable=False),
            sa.Column('policy_id', sa.Integer, nullable=False),
            sa.Column('job_id', sa.String(36), nullable=True),
            sa.Column('kind', sa.String(20), nullable=False),
            sa.Column('status', sa.String(20), nullable=False),
            sa.Column('started_at', sa.DateTime, nullable=True),
            sa.Column('finished_at', sa.DateTime, nullable=True),
            sa.Column('duration_seconds', sa.Integer, nullable=True),
            sa.Column('size_local', sa.BigInteger, nullable=True, server_default='0'),
            sa.Column('size_remote', sa.BigInteger, nullable=True, server_default='0'),
            sa.Column('cost_local', sa.Numeric(10, 4), nullable=True, server_default='0'),
            sa.Column('cost_remote', sa.Numeric(10, 4), nullable=True, server_default='0'),
            sa.Column('storage_path', sa.Text, nullable=True),
            sa.Column('remote_key', sa.Text, nullable=True),
            sa.Column('verified', sa.Boolean, nullable=True, server_default=sa.false()),
            sa.Column('error_message', sa.Text, nullable=True),
            sa.Column('metadata_json', sa.Text, nullable=True, server_default='{}'),
            sa.PrimaryKeyConstraint('id'),
            sa.ForeignKeyConstraint(['policy_id'], ['backup_policies.id']),
        )
        op.create_index('ix_backup_runs_policy_id', 'backup_runs', ['policy_id'])
        op.create_index('ix_backup_runs_job_id', 'backup_runs', ['job_id'])


def downgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing = set(inspector.get_table_names())
    if 'backup_runs' in existing:
        op.drop_table('backup_runs')
    if 'backup_policies' in existing:
        op.drop_table('backup_policies')
