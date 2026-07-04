"""Baseline migration capturing full schema.

Revision ID: 001_baseline
Revises:
Create Date: 2026-03-04

For fresh installs: creates all tables from scratch.
For existing DBs: acts as a stamp point (tables already exist).
"""
from alembic import op
import sqlalchemy as sa

revision = '001_baseline'
down_revision = None
branch_labels = None
depends_on = None


def upgrade():
    # Use batch mode and check if tables already exist to support both
    # fresh installs and existing databases being stamped.
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing_tables = inspector.get_table_names()

    if 'users' not in existing_tables:
        op.create_table('users',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('email', sa.String(120), unique=True, nullable=False, index=True),
            sa.Column('username', sa.String(80), unique=True, nullable=False, index=True),
            sa.Column('password_hash', sa.String(256), nullable=True),
            sa.Column('auth_provider', sa.String(50), server_default='local'),
            sa.Column('role', sa.String(20), server_default='developer'),
            sa.Column('is_active', sa.Boolean(), server_default=sa.text('1')),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('updated_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('last_login_at', sa.DateTime(), nullable=True),
            sa.Column('created_by', sa.Integer(), sa.ForeignKey('users.id'), nullable=True),
            sa.Column('failed_login_count', sa.Integer(), server_default='0'),
            sa.Column('locked_until', sa.DateTime(), nullable=True),
            sa.Column('totp_secret', sa.String(32), nullable=True),
            sa.Column('totp_enabled', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('backup_codes', sa.Text(), nullable=True),
            sa.Column('totp_confirmed_at', sa.DateTime(), nullable=True),
        )
    else:
        # Add columns that may be missing in existing installs
        existing_cols = {c['name'] for c in inspector.get_columns('users')}
        if 'auth_provider' not in existing_cols:
            with op.batch_alter_table('users') as batch_op:
                batch_op.add_column(sa.Column('auth_provider', sa.String(50), server_default='local'))

    if 'applications' not in existing_tables:
        op.create_table('applications',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('name', sa.String(100), nullable=False),
            sa.Column('app_type', sa.String(50), nullable=False),
            sa.Column('status', sa.String(20), server_default='stopped'),
            sa.Column('php_version', sa.String(10), nullable=True),
            sa.Column('python_version', sa.String(10), nullable=True),
            sa.Column('port', sa.Integer(), nullable=True),
            sa.Column('root_path', sa.String(500), nullable=True),
            sa.Column('docker_image', sa.String(200), nullable=True),
            sa.Column('container_id', sa.String(100), nullable=True),
            sa.Column('private_slug', sa.String(50), unique=True, nullable=True, index=True),
            sa.Column('private_url_enabled', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('environment_type', sa.String(20), server_default='standalone'),
            sa.Column('linked_app_id', sa.Integer(), sa.ForeignKey('applications.id'), nullable=True),
            sa.Column('shared_config', sa.Text(), nullable=True),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('updated_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('last_deployed_at', sa.DateTime(), nullable=True),
            sa.Column('user_id', sa.Integer(), sa.ForeignKey('users.id'), nullable=False),
        )

    if 'domains' not in existing_tables:
        op.create_table('domains',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('name', sa.String(255), unique=True, nullable=False, index=True),
            sa.Column('is_primary', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('ssl_enabled', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('ssl_certificate_path', sa.String(500), nullable=True),
            sa.Column('ssl_key_path', sa.String(500), nullable=True),
            sa.Column('ssl_expires_at', sa.DateTime(), nullable=True),
            sa.Column('ssl_auto_renew', sa.Boolean(), server_default=sa.text('1')),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('updated_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('application_id', sa.Integer(), sa.ForeignKey('applications.id'), nullable=False),
        )

    if 'environment_variables' not in existing_tables:
        op.create_table('environment_variables',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('application_id', sa.Integer(), sa.ForeignKey('applications.id'), nullable=False),
            sa.Column('key', sa.String(255), nullable=False),
            sa.Column('encrypted_value', sa.Text(), nullable=False),
            sa.Column('is_secret', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('description', sa.String(500), nullable=True),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('updated_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('created_by', sa.Integer(), sa.ForeignKey('users.id'), nullable=True),
            sa.UniqueConstraint('application_id', 'key', name='unique_app_env_key'),
        )

    if 'environment_variable_history' not in existing_tables:
        op.create_table('environment_variable_history',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('env_variable_id', sa.Integer(), nullable=False),
            sa.Column('application_id', sa.Integer(), sa.ForeignKey('applications.id'), nullable=False),
            sa.Column('key', sa.String(255), nullable=False),
            sa.Column('action', sa.String(20), nullable=False),
            sa.Column('old_value_hash', sa.String(64), nullable=True),
            sa.Column('new_value_hash', sa.String(64), nullable=True),
            sa.Column('changed_by', sa.Integer(), sa.ForeignKey('users.id'), nullable=True),
            sa.Column('changed_at', sa.DateTime(), server_default=sa.func.now()),
        )

    if 'notification_preferences' not in existing_tables:
        op.create_table('notification_preferences',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('user_id', sa.Integer(), sa.ForeignKey('users.id'), unique=True, nullable=False),
            sa.Column('enabled', sa.Boolean(), server_default=sa.text('1')),
            sa.Column('channels', sa.Text(), server_default='["email"]'),
            sa.Column('severities', sa.Text(), server_default='["critical", "warning"]'),
            sa.Column('email', sa.String(255), nullable=True),
            sa.Column('discord_webhook', sa.String(512), nullable=True),
            sa.Column('telegram_chat_id', sa.String(64), nullable=True),
            sa.Column('categories', sa.Text(), server_default='{"system": true, "security": true, "backups": true, "apps": true}'),
            sa.Column('quiet_hours_enabled', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('quiet_hours_start', sa.String(5), server_default='22:00'),
            sa.Column('quiet_hours_end', sa.String(5), server_default='08:00'),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('updated_at', sa.DateTime(), server_default=sa.func.now()),
        )

    if 'deployments' not in existing_tables:
        op.create_table('deployments',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('app_id', sa.Integer(), sa.ForeignKey('applications.id'), nullable=False),
            sa.Column('version', sa.Integer(), nullable=False),
            sa.Column('version_tag', sa.String(100), nullable=True),
            sa.Column('status', sa.String(20), server_default='pending'),
            sa.Column('build_method', sa.String(20), nullable=True),
            sa.Column('image_tag', sa.String(255), nullable=True),
            sa.Column('commit_hash', sa.String(40), nullable=True),
            sa.Column('commit_message', sa.Text(), nullable=True),
            sa.Column('container_id', sa.String(100), nullable=True),
            sa.Column('deployed_by', sa.Integer(), sa.ForeignKey('users.id'), nullable=True),
            sa.Column('deploy_trigger', sa.String(20), server_default='manual'),
            sa.Column('build_log_path', sa.String(500), nullable=True),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('build_started_at', sa.DateTime(), nullable=True),
            sa.Column('build_completed_at', sa.DateTime(), nullable=True),
            sa.Column('deploy_started_at', sa.DateTime(), nullable=True),
            sa.Column('deploy_completed_at', sa.DateTime(), nullable=True),
            sa.Column('error_message', sa.Text(), nullable=True),
            sa.Column('extra_data', sa.Text(), server_default='{}'),
        )

    if 'deployment_diffs' not in existing_tables:
        op.create_table('deployment_diffs',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('deployment_id', sa.Integer(), sa.ForeignKey('deployments.id'), nullable=False),
            sa.Column('previous_deployment_id', sa.Integer(), sa.ForeignKey('deployments.id'), nullable=True),
            sa.Column('files_added', sa.Text(), server_default='[]'),
            sa.Column('files_removed', sa.Text(), server_default='[]'),
            sa.Column('files_modified', sa.Text(), server_default='[]'),
            sa.Column('additions', sa.Integer(), server_default='0'),
            sa.Column('deletions', sa.Integer(), server_default='0'),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
        )

    if 'system_settings' not in existing_tables:
        op.create_table('system_settings',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('key', sa.String(100), unique=True, nullable=False, index=True),
            sa.Column('value', sa.Text(), nullable=True),
            sa.Column('value_type', sa.String(20), server_default='string'),
            sa.Column('description', sa.String(500), nullable=True),
            sa.Column('updated_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('updated_by', sa.Integer(), sa.ForeignKey('users.id'), nullable=True),
        )

    if 'audit_logs' not in existing_tables:
        op.create_table('audit_logs',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('action', sa.String(100), nullable=False, index=True),
            sa.Column('user_id', sa.Integer(), sa.ForeignKey('users.id'), nullable=True),
            sa.Column('target_type', sa.String(50), nullable=True),
            sa.Column('target_id', sa.Integer(), nullable=True),
            sa.Column('details', sa.Text(), nullable=True),
            sa.Column('ip_address', sa.String(45), nullable=True),
            sa.Column('user_agent', sa.String(500), nullable=True),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now(), index=True),
        )

    if 'metrics_history' not in existing_tables:
        op.create_table('metrics_history',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('timestamp', sa.DateTime(), nullable=False, index=True),
            sa.Column('level', sa.String(10), nullable=False, server_default='minute', index=True),
            sa.Column('cpu_percent', sa.Float(), nullable=False),
            sa.Column('cpu_percent_min', sa.Float(), nullable=True),
            sa.Column('cpu_percent_max', sa.Float(), nullable=True),
            sa.Column('memory_percent', sa.Float(), nullable=False),
            sa.Column('memory_used_bytes', sa.BigInteger(), nullable=False),
            sa.Column('memory_total_bytes', sa.BigInteger(), nullable=False),
            sa.Column('disk_percent', sa.Float(), nullable=False),
            sa.Column('disk_used_bytes', sa.BigInteger(), nullable=False),
            sa.Column('disk_total_bytes', sa.BigInteger(), nullable=False),
            sa.Column('load_1m', sa.Float(), nullable=True),
            sa.Column('load_5m', sa.Float(), nullable=True),
            sa.Column('load_15m', sa.Float(), nullable=True),
            sa.Column('sample_count', sa.Integer(), server_default='1'),
        )
        op.create_index('idx_metrics_level_timestamp', 'metrics_history', ['level', 'timestamp'])

    if 'workflows' not in existing_tables:
        op.create_table('workflows',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('name', sa.String(100), nullable=False),
            sa.Column('description', sa.Text(), nullable=True),
            sa.Column('nodes', sa.Text(), nullable=True),
            sa.Column('edges', sa.Text(), nullable=True),
            sa.Column('viewport', sa.Text(), nullable=True),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('updated_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('user_id', sa.Integer(), sa.ForeignKey('users.id'), nullable=False),
        )

    if 'git_webhooks' not in existing_tables:
        op.create_table('git_webhooks',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('name', sa.String(100), nullable=False),
            sa.Column('source', sa.String(50), nullable=False),
            sa.Column('source_repo_url', sa.String(500), nullable=False),
            sa.Column('source_branch', sa.String(100), server_default='main'),
            sa.Column('local_repo_name', sa.String(200), nullable=True),
            sa.Column('secret', sa.String(100), nullable=False),
            sa.Column('webhook_token', sa.String(50), nullable=False, unique=True),
            sa.Column('sync_direction', sa.String(20), server_default='pull'),
            sa.Column('auto_sync', sa.Boolean(), server_default=sa.text('1')),
            sa.Column('app_id', sa.Integer(), sa.ForeignKey('applications.id'), nullable=True),
            sa.Column('deploy_on_push', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('pre_deploy_script', sa.Text(), nullable=True),
            sa.Column('post_deploy_script', sa.Text(), nullable=True),
            sa.Column('zero_downtime', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('is_active', sa.Boolean(), server_default=sa.text('1')),
            sa.Column('last_sync_at', sa.DateTime(), nullable=True),
            sa.Column('last_sync_status', sa.String(20), nullable=True),
            sa.Column('last_sync_message', sa.Text(), nullable=True),
            sa.Column('sync_count', sa.Integer(), server_default='0'),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('updated_at', sa.DateTime(), server_default=sa.func.now()),
        )

    if 'webhook_logs' not in existing_tables:
        op.create_table('webhook_logs',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('webhook_id', sa.Integer(), sa.ForeignKey('git_webhooks.id'), nullable=True),
            sa.Column('source', sa.String(50), nullable=False),
            sa.Column('event_type', sa.String(50), nullable=False),
            sa.Column('delivery_id', sa.String(100), nullable=True),
            sa.Column('ref', sa.String(200), nullable=True),
            sa.Column('commit_sha', sa.String(64), nullable=True),
            sa.Column('commit_message', sa.Text(), nullable=True),
            sa.Column('pusher', sa.String(100), nullable=True),
            sa.Column('status', sa.String(20), server_default='received'),
            sa.Column('status_message', sa.Text(), nullable=True),
            sa.Column('headers_json', sa.Text(), nullable=True),
            sa.Column('payload_preview', sa.Text(), nullable=True),
            sa.Column('received_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('processed_at', sa.DateTime(), nullable=True),
        )

    if 'git_deployments' not in existing_tables:
        op.create_table('git_deployments',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('app_id', sa.Integer(), sa.ForeignKey('applications.id'), nullable=False),
            sa.Column('webhook_id', sa.Integer(), sa.ForeignKey('git_webhooks.id'), nullable=True),
            sa.Column('version', sa.Integer(), nullable=False),
            sa.Column('commit_sha', sa.String(64), nullable=True),
            sa.Column('commit_message', sa.Text(), nullable=True),
            sa.Column('branch', sa.String(100), nullable=True),
            sa.Column('triggered_by', sa.String(100), nullable=True),
            sa.Column('status', sa.String(20), server_default='pending'),
            sa.Column('started_at', sa.DateTime(), nullable=True),
            sa.Column('completed_at', sa.DateTime(), nullable=True),
            sa.Column('duration_seconds', sa.Integer(), nullable=True),
            sa.Column('pre_script_output', sa.Text(), nullable=True),
            sa.Column('deploy_output', sa.Text(), nullable=True),
            sa.Column('post_script_output', sa.Text(), nullable=True),
            sa.Column('error_message', sa.Text(), nullable=True),
            sa.Column('is_rollback', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('rollback_from_version', sa.Integer(), nullable=True),
            sa.Column('rolled_back_at', sa.DateTime(), nullable=True),
            sa.Column('rolled_back_to_id', sa.Integer(), nullable=True),
            sa.Column('snapshot_data', sa.Text(), nullable=True),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
        )

    if 'server_groups' not in existing_tables:
        op.create_table('server_groups',
            sa.Column('id', sa.String(36), primary_key=True),
            sa.Column('name', sa.String(100), nullable=False),
            sa.Column('description', sa.Text()),
            sa.Column('color', sa.String(7), server_default='#6366f1'),
            sa.Column('icon', sa.String(50), server_default='server'),
            sa.Column('parent_id', sa.String(36), sa.ForeignKey('server_groups.id'), nullable=True),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('updated_at', sa.DateTime(), server_default=sa.func.now()),
        )

    if 'servers' not in existing_tables:
        op.create_table('servers',
            sa.Column('id', sa.String(36), primary_key=True),
            sa.Column('name', sa.String(100), nullable=False),
            sa.Column('description', sa.Text()),
            sa.Column('hostname', sa.String(255)),
            sa.Column('ip_address', sa.String(45)),
            sa.Column('group_id', sa.String(36), sa.ForeignKey('server_groups.id'), nullable=True),
            sa.Column('tags', sa.JSON()),
            sa.Column('status', sa.String(20), server_default='pending'),
            sa.Column('last_seen', sa.DateTime()),
            sa.Column('last_error', sa.Text()),
            sa.Column('agent_version', sa.String(20)),
            sa.Column('agent_id', sa.String(36), unique=True, index=True),
            sa.Column('os_type', sa.String(20)),
            sa.Column('os_version', sa.String(100)),
            sa.Column('platform', sa.String(100)),
            sa.Column('architecture', sa.String(20)),
            sa.Column('cpu_cores', sa.Integer()),
            sa.Column('cpu_model', sa.String(200)),
            sa.Column('total_memory', sa.BigInteger()),
            sa.Column('total_disk', sa.BigInteger()),
            sa.Column('docker_version', sa.String(50)),
            sa.Column('api_key_hash', sa.String(256)),
            sa.Column('api_key_prefix', sa.String(12)),
            sa.Column('api_secret_encrypted', sa.Text()),
            sa.Column('permissions', sa.JSON()),
            sa.Column('allowed_ips', sa.JSON()),
            sa.Column('api_key_pending_hash', sa.String(256)),
            sa.Column('api_key_pending_prefix', sa.String(12)),
            sa.Column('api_secret_pending_encrypted', sa.Text()),
            sa.Column('api_key_rotation_expires', sa.DateTime()),
            sa.Column('api_key_rotation_id', sa.String(36)),
            sa.Column('api_key_last_rotated', sa.DateTime()),
            sa.Column('registration_token_hash', sa.String(256)),
            sa.Column('registration_token_expires', sa.DateTime()),
            sa.Column('registered_at', sa.DateTime()),
            sa.Column('registered_by', sa.Integer(), sa.ForeignKey('users.id')),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('updated_at', sa.DateTime(), server_default=sa.func.now()),
        )

    if 'server_metrics' not in existing_tables:
        op.create_table('server_metrics',
            sa.Column('id', sa.BigInteger(), primary_key=True, autoincrement=True),
            sa.Column('server_id', sa.String(36), sa.ForeignKey('servers.id'), nullable=False, index=True),
            sa.Column('timestamp', sa.DateTime(), server_default=sa.func.now(), index=True),
            sa.Column('cpu_percent', sa.Float()),
            sa.Column('memory_percent', sa.Float()),
            sa.Column('memory_used', sa.BigInteger()),
            sa.Column('disk_percent', sa.Float()),
            sa.Column('disk_used', sa.BigInteger()),
            sa.Column('network_rx', sa.BigInteger()),
            sa.Column('network_tx', sa.BigInteger()),
            sa.Column('network_rx_rate', sa.Float()),
            sa.Column('network_tx_rate', sa.Float()),
            sa.Column('container_count', sa.Integer()),
            sa.Column('container_running', sa.Integer()),
            sa.Column('extra', sa.JSON()),
        )
        op.create_index('ix_server_metrics_server_time', 'server_metrics', ['server_id', 'timestamp'])

    if 'server_commands' not in existing_tables:
        op.create_table('server_commands',
            sa.Column('id', sa.String(36), primary_key=True),
            sa.Column('server_id', sa.String(36), sa.ForeignKey('servers.id'), nullable=False, index=True),
            sa.Column('user_id', sa.Integer(), sa.ForeignKey('users.id')),
            sa.Column('command_type', sa.String(50)),
            sa.Column('command_data', sa.JSON()),
            sa.Column('status', sa.String(20), server_default='pending'),
            sa.Column('started_at', sa.DateTime()),
            sa.Column('completed_at', sa.DateTime()),
            sa.Column('result', sa.JSON()),
            sa.Column('error', sa.Text()),
            sa.Column('exit_code', sa.Integer()),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
        )

    if 'agent_sessions' not in existing_tables:
        op.create_table('agent_sessions',
            sa.Column('id', sa.String(36), primary_key=True),
            sa.Column('server_id', sa.String(36), sa.ForeignKey('servers.id'), nullable=False, index=True),
            sa.Column('session_token', sa.String(256)),
            sa.Column('socket_id', sa.String(100)),
            sa.Column('connected_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('last_heartbeat', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('ip_address', sa.String(45)),
            sa.Column('user_agent', sa.String(255)),
            sa.Column('is_active', sa.Boolean(), server_default=sa.text('1'), index=True),
            sa.Column('disconnected_at', sa.DateTime()),
            sa.Column('disconnect_reason', sa.String(100)),
        )

    if 'security_alerts' not in existing_tables:
        op.create_table('security_alerts',
            sa.Column('id', sa.String(36), primary_key=True),
            sa.Column('server_id', sa.String(36), sa.ForeignKey('servers.id'), nullable=True, index=True),
            sa.Column('alert_type', sa.String(50), nullable=False, index=True),
            sa.Column('severity', sa.String(20), nullable=False, server_default='info', index=True),
            sa.Column('source_ip', sa.String(45)),
            sa.Column('details', sa.JSON()),
            sa.Column('status', sa.String(20), server_default='open', index=True),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now(), index=True),
            sa.Column('acknowledged_at', sa.DateTime()),
            sa.Column('acknowledged_by', sa.Integer(), sa.ForeignKey('users.id')),
            sa.Column('resolved_at', sa.DateTime()),
            sa.Column('resolved_by', sa.Integer(), sa.ForeignKey('users.id')),
        )

    if 'wordpress_sites' not in existing_tables:
        op.create_table('wordpress_sites',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('application_id', sa.Integer(), sa.ForeignKey('applications.id'), unique=True, nullable=False),
            sa.Column('wp_version', sa.String(20)),
            sa.Column('multisite', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('admin_user', sa.String(100)),
            sa.Column('admin_email', sa.String(200)),
            sa.Column('db_name', sa.String(100)),
            sa.Column('db_user', sa.String(100)),
            sa.Column('db_host', sa.String(200), server_default='localhost'),
            sa.Column('db_prefix', sa.String(20), server_default='wp_'),
            sa.Column('git_repo_url', sa.String(500)),
            sa.Column('git_branch', sa.String(100), server_default='main'),
            sa.Column('git_paths', sa.Text()),
            sa.Column('auto_deploy', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('last_deploy_commit', sa.String(40)),
            sa.Column('last_deploy_at', sa.DateTime()),
            sa.Column('is_production', sa.Boolean(), server_default=sa.text('1')),
            sa.Column('production_site_id', sa.Integer(), sa.ForeignKey('wordpress_sites.id'), nullable=True),
            sa.Column('sync_config', sa.Text()),
            sa.Column('environment_type', sa.String(20), server_default='standalone'),
            sa.Column('multidev_branch', sa.String(200)),
            sa.Column('is_locked', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('locked_by', sa.String(100)),
            sa.Column('locked_reason', sa.String(200)),
            sa.Column('lock_expires_at', sa.DateTime()),
            sa.Column('compose_project_name', sa.String(100)),
            sa.Column('container_prefix', sa.String(100)),
            sa.Column('resource_limits', sa.Text()),
            sa.Column('basic_auth_enabled', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('basic_auth_user', sa.String(100)),
            sa.Column('basic_auth_password_hash', sa.String(200)),
            sa.Column('health_status', sa.String(20), server_default='unknown'),
            sa.Column('last_health_check', sa.DateTime()),
            sa.Column('disk_usage_bytes', sa.BigInteger(), server_default='0'),
            sa.Column('disk_usage_updated_at', sa.DateTime()),
            sa.Column('auto_sync_schedule', sa.String(100)),
            sa.Column('auto_sync_enabled', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('updated_at', sa.DateTime(), server_default=sa.func.now()),
        )

    if 'database_snapshots' not in existing_tables:
        op.create_table('database_snapshots',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('site_id', sa.Integer(), sa.ForeignKey('wordpress_sites.id'), nullable=False),
            sa.Column('name', sa.String(200), nullable=False),
            sa.Column('description', sa.Text()),
            sa.Column('tag', sa.String(100)),
            sa.Column('file_path', sa.String(500), nullable=False),
            sa.Column('size_bytes', sa.BigInteger(), server_default='0'),
            sa.Column('compressed', sa.Boolean(), server_default=sa.text('1')),
            sa.Column('commit_sha', sa.String(40)),
            sa.Column('commit_message', sa.Text()),
            sa.Column('tables_included', sa.Text()),
            sa.Column('row_count', sa.Integer()),
            sa.Column('status', sa.String(20), server_default='completed'),
            sa.Column('error_message', sa.Text()),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('expires_at', sa.DateTime()),
        )

    if 'sync_jobs' not in existing_tables:
        op.create_table('sync_jobs',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('source_site_id', sa.Integer(), sa.ForeignKey('wordpress_sites.id'), nullable=False),
            sa.Column('target_site_id', sa.Integer(), sa.ForeignKey('wordpress_sites.id'), nullable=False),
            sa.Column('name', sa.String(200)),
            sa.Column('schedule', sa.String(100)),
            sa.Column('enabled', sa.Boolean(), server_default=sa.text('1')),
            sa.Column('config', sa.Text()),
            sa.Column('last_run', sa.DateTime()),
            sa.Column('last_run_status', sa.String(20)),
            sa.Column('last_run_duration', sa.Integer()),
            sa.Column('last_run_error', sa.Text()),
            sa.Column('next_run', sa.DateTime()),
            sa.Column('run_count', sa.Integer(), server_default='0'),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('updated_at', sa.DateTime(), server_default=sa.func.now()),
        )

    if 'environment_activities' not in existing_tables:
        op.create_table('environment_activities',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('site_id', sa.Integer(), sa.ForeignKey('wordpress_sites.id'), nullable=False),
            sa.Column('user_id', sa.Integer(), sa.ForeignKey('users.id'), nullable=True),
            sa.Column('action', sa.String(50), nullable=False),
            sa.Column('description', sa.Text()),
            sa.Column('metadata', sa.Text()),
            sa.Column('status', sa.String(20), server_default='completed'),
            sa.Column('error_message', sa.Text()),
            sa.Column('duration_seconds', sa.Float()),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
        )

    if 'promotion_jobs' not in existing_tables:
        op.create_table('promotion_jobs',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('source_site_id', sa.Integer(), sa.ForeignKey('wordpress_sites.id'), nullable=False),
            sa.Column('target_site_id', sa.Integer(), sa.ForeignKey('wordpress_sites.id'), nullable=False),
            sa.Column('user_id', sa.Integer(), sa.ForeignKey('users.id'), nullable=True),
            sa.Column('promotion_type', sa.String(20), nullable=False),
            sa.Column('config', sa.Text()),
            sa.Column('status', sa.String(20), server_default='pending'),
            sa.Column('pre_promotion_snapshot_id', sa.Integer(), sa.ForeignKey('database_snapshots.id'), nullable=True),
            sa.Column('error_message', sa.Text()),
            sa.Column('started_at', sa.DateTime()),
            sa.Column('completed_at', sa.DateTime()),
            sa.Column('duration_seconds', sa.Float()),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
        )

    if 'sanitization_profiles' not in existing_tables:
        op.create_table('sanitization_profiles',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('user_id', sa.Integer(), sa.ForeignKey('users.id'), nullable=False),
            sa.Column('name', sa.String(100), nullable=False),
            sa.Column('description', sa.Text()),
            sa.Column('config', sa.Text(), nullable=False),
            sa.Column('is_default', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('is_builtin', sa.Boolean(), server_default=sa.text('0')),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('updated_at', sa.DateTime()),
        )

    if 'email_accounts' not in existing_tables:
        op.create_table('email_accounts',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('email', sa.String(255), unique=True, nullable=False),
            sa.Column('domain', sa.String(255), nullable=False),
            sa.Column('username', sa.String(100), nullable=False),
            sa.Column('quota_mb', sa.Integer(), server_default='1024'),
            sa.Column('enabled', sa.Boolean(), server_default=sa.text('1')),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('updated_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('forward_to', sa.Text(), nullable=True),
            sa.Column('forward_keep_copy', sa.Boolean(), server_default=sa.text('1')),
        )

    if 'oauth_identities' not in existing_tables:
        op.create_table('oauth_identities',
            sa.Column('id', sa.Integer(), primary_key=True),
            sa.Column('user_id', sa.Integer(), sa.ForeignKey('users.id', ondelete='CASCADE'), nullable=False, index=True),
            sa.Column('provider', sa.String(50), nullable=False),
            sa.Column('provider_user_id', sa.String(256), nullable=False),
            sa.Column('provider_email', sa.String(256), nullable=True),
            sa.Column('provider_display_name', sa.String(256), nullable=True),
            sa.Column('access_token_encrypted', sa.Text(), nullable=True),
            sa.Column('refresh_token_encrypted', sa.Text(), nullable=True),
            sa.Column('token_expires_at', sa.DateTime(), nullable=True),
            sa.Column('created_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('updated_at', sa.DateTime(), server_default=sa.func.now()),
            sa.Column('last_login_at', sa.DateTime(), nullable=True),
            sa.UniqueConstraint('provider', 'provider_user_id', name='uq_provider_identity'),
        )


def downgrade():
    # Drop tables in reverse dependency order
    tables = [
        'oauth_identities', 'email_accounts', 'sanitization_profiles',
        'promotion_jobs', 'environment_activities', 'sync_jobs',
        'database_snapshots', 'wordpress_sites', 'security_alerts',
        'agent_sessions', 'server_commands', 'server_metrics', 'servers',
        'server_groups', 'git_deployments', 'webhook_logs', 'git_webhooks',
        'workflows', 'metrics_history', 'audit_logs', 'system_settings',
        'deployment_diffs', 'deployments', 'notification_preferences',
        'environment_variable_history', 'environment_variables', 'domains',
        'applications', 'users',
    ]

    conn = op.get_bind()
    inspector = sa.inspect(conn)
    existing_tables = inspector.get_table_names()

    for table in tables:
        if table in existing_tables:
            op.drop_table(table)
