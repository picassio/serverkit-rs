CREATE TABLE users (
	id INTEGER NOT NULL, 
	email VARCHAR(120) NOT NULL, 
	username VARCHAR(80) NOT NULL, 
	password_hash VARCHAR(256), 
	auth_provider VARCHAR(50), 
	role VARCHAR(20), 
	permissions TEXT, 
	is_active BOOLEAN, 
	created_at DATETIME, 
	updated_at DATETIME, 
	last_login_at DATETIME, 
	created_by INTEGER, 
	failed_login_count INTEGER, 
	locked_until DATETIME, 
	totp_secret VARCHAR(32), 
	totp_enabled BOOLEAN, 
	backup_codes TEXT, 
	totp_confirmed_at DATETIME, 
	sidebar_config TEXT, 
	PRIMARY KEY (id), 
	FOREIGN KEY(created_by) REFERENCES users (id)
);
CREATE UNIQUE INDEX ix_users_username ON users (username);
CREATE INDEX ix_users_created_at ON users (created_at);
CREATE UNIQUE INDEX ix_users_email ON users (email);
CREATE INDEX ix_users_auth_provider ON users (auth_provider);
CREATE TABLE resource_tags (
	id INTEGER NOT NULL, 
	resource_type VARCHAR(50) NOT NULL, 
	resource_id VARCHAR(255) NOT NULL, 
	tag VARCHAR(100) NOT NULL, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_resource_tag UNIQUE (resource_type, resource_id, tag)
);
CREATE INDEX ix_resource_tag_resource ON resource_tags (resource_type, resource_id);
CREATE TABLE shared_variable_groups (
	id INTEGER NOT NULL, 
	scope_type VARCHAR(50) NOT NULL, 
	scope_id VARCHAR(255) NOT NULL, 
	name VARCHAR(255) NOT NULL, 
	description VARCHAR(500), 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id)
);
CREATE INDEX ix_shared_group_scope ON shared_variable_groups (scope_type, scope_id);
CREATE TABLE metrics_history (
	id INTEGER NOT NULL, 
	timestamp DATETIME NOT NULL, 
	level VARCHAR(10) NOT NULL, 
	cpu_percent FLOAT NOT NULL, 
	cpu_percent_min FLOAT, 
	cpu_percent_max FLOAT, 
	memory_percent FLOAT NOT NULL, 
	memory_used_bytes BIGINT NOT NULL, 
	memory_total_bytes BIGINT NOT NULL, 
	disk_percent FLOAT NOT NULL, 
	disk_used_bytes BIGINT NOT NULL, 
	disk_total_bytes BIGINT NOT NULL, 
	load_1m FLOAT, 
	load_5m FLOAT, 
	load_15m FLOAT, 
	sample_count INTEGER, 
	PRIMARY KEY (id)
);
CREATE INDEX ix_metrics_history_timestamp ON metrics_history (timestamp);
CREATE INDEX ix_metrics_history_level ON metrics_history (level);
CREATE INDEX idx_metrics_level_timestamp ON metrics_history (level, timestamp);
CREATE TABLE server_groups (
	id VARCHAR(36) NOT NULL, 
	name VARCHAR(100) NOT NULL, 
	description TEXT, 
	color VARCHAR(7), 
	icon VARCHAR(50), 
	parent_id VARCHAR(36), 
	auto_upgrade BOOLEAN, 
	upgrade_channel VARCHAR(20), 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(parent_id) REFERENCES server_groups (id)
);
CREATE TABLE agent_versions (
	id VARCHAR(36) NOT NULL, 
	version VARCHAR(20) NOT NULL, 
	channel VARCHAR(20), 
	min_panel_version VARCHAR(20), 
	max_panel_version VARCHAR(20), 
	release_notes TEXT, 
	is_active BOOLEAN, 
	published_at DATETIME, 
	assets JSON, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	UNIQUE (version)
);
CREATE TABLE server_onboarding_logs (
	id INTEGER NOT NULL, 
	server_id VARCHAR(36) NOT NULL, 
	state VARCHAR(40) NOT NULL, 
	status VARCHAR(20) NOT NULL, 
	message TEXT, 
	detail_json TEXT, 
	created_at DATETIME, 
	PRIMARY KEY (id)
);
CREATE INDEX ix_server_onboarding_logs_server_id ON server_onboarding_logs (server_id);
CREATE INDEX ix_server_onboarding_logs_created_at ON server_onboarding_logs (created_at);
CREATE TABLE wordpress_custom_plugins (
	id INTEGER NOT NULL, 
	slug VARCHAR(200) NOT NULL, 
	name VARCHAR(255), 
	description TEXT, 
	version VARCHAR(50), 
	author VARCHAR(255), 
	source_type VARCHAR(20) NOT NULL, 
	source_url VARCHAR(500) NOT NULL, 
	branch VARCHAR(100), 
	connection_id INTEGER, 
	is_active BOOLEAN, 
	last_synced_at DATETIME, 
	sync_error TEXT, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id)
);
CREATE UNIQUE INDEX ix_wordpress_custom_plugins_slug ON wordpress_custom_plugins (slug);
CREATE TABLE dns_provider_configs (
	id INTEGER NOT NULL, 
	name VARCHAR(100) NOT NULL, 
	provider VARCHAR(50) NOT NULL, 
	api_key VARCHAR(500), 
	api_secret VARCHAR(500), 
	api_email VARCHAR(255), 
	is_default BOOLEAN, 
	created_at DATETIME, 
	PRIMARY KEY (id)
);
CREATE TABLE email_relay_config (
	id INTEGER NOT NULL, 
	enabled BOOLEAN, 
	host VARCHAR(255), 
	port INTEGER, 
	username VARCHAR(255), 
	password_encrypted TEXT, 
	use_tls BOOLEAN, 
	provider_hint VARCHAR(40), 
	updated_at DATETIME, 
	PRIMARY KEY (id)
);
CREATE TABLE agent_plugins (
	id INTEGER NOT NULL, 
	name VARCHAR(128) NOT NULL, 
	display_name VARCHAR(256) NOT NULL, 
	version VARCHAR(32) NOT NULL, 
	description TEXT, 
	author VARCHAR(128), 
	homepage VARCHAR(512), 
	manifest_json TEXT, 
	capabilities_json TEXT, 
	dependencies_json TEXT, 
	permissions_json TEXT, 
	max_memory_mb INTEGER, 
	max_cpu_percent INTEGER, 
	status VARCHAR(32), 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	UNIQUE (name)
);
CREATE TABLE status_pages (
	id INTEGER NOT NULL, 
	name VARCHAR(128) NOT NULL, 
	slug VARCHAR(128) NOT NULL, 
	description TEXT, 
	logo_url VARCHAR(512), 
	primary_color VARCHAR(7), 
	custom_domain VARCHAR(256), 
	is_public BOOLEAN, 
	show_uptime BOOLEAN, 
	show_history BOOLEAN, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	UNIQUE (slug)
);
CREATE TABLE backup_policies (
	id INTEGER NOT NULL, 
	target_type VARCHAR(40) NOT NULL, 
	target_id INTEGER NOT NULL, 
	target_subtype VARCHAR(40), 
	target_meta_json TEXT, 
	enabled BOOLEAN NOT NULL, 
	schedule_cron VARCHAR(120) NOT NULL, 
	retention_count INTEGER NOT NULL, 
	retention_days INTEGER NOT NULL, 
	full_every_n_days INTEGER NOT NULL, 
	compression VARCHAR(20) NOT NULL, 
	remote_copy BOOLEAN NOT NULL, 
	pre_backup_hook TEXT, 
	post_backup_hook TEXT, 
	last_run_at DATETIME, 
	last_status VARCHAR(20), 
	last_size BIGINT, 
	last_cost_local NUMERIC(10, 4), 
	last_cost_remote NUMERIC(10, 4), 
	last_job_id VARCHAR(36), 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_backup_policy_target UNIQUE (target_type, target_id)
);
CREATE INDEX ix_backup_policies_target_type ON backup_policies (target_type);
CREATE INDEX ix_backup_policies_target_id ON backup_policies (target_id);
CREATE TABLE queue_groups (
	id VARCHAR(36) NOT NULL, 
	slug VARCHAR(128) NOT NULL, 
	name VARCHAR(256) NOT NULL, 
	description TEXT, 
	owner_type VARCHAR(32) NOT NULL, 
	owner_id VARCHAR(128), 
	config_json TEXT, 
	created_at DATETIME NOT NULL, 
	updated_at DATETIME NOT NULL, 
	PRIMARY KEY (id)
);
CREATE UNIQUE INDEX ix_queue_groups_slug ON queue_groups (slug);
CREATE TABLE notifications (
	id INTEGER NOT NULL, 
	event_key VARCHAR(120) NOT NULL, 
	category VARCHAR(40), 
	severity VARCHAR(20), 
	title VARCHAR(255) NOT NULL, 
	body TEXT, 
	data_json TEXT, 
	audience VARCHAR(255), 
	correlation_id VARCHAR(64), 
	created_at DATETIME, 
	PRIMARY KEY (id)
);
CREATE INDEX ix_notifications_category ON notifications (category);
CREATE INDEX ix_notifications_correlation_id ON notifications (correlation_id);
CREATE INDEX ix_notifications_severity ON notifications (severity);
CREATE INDEX ix_notifications_created_at ON notifications (created_at);
CREATE INDEX ix_notifications_event_key ON notifications (event_key);
CREATE TABLE domain_registrations (
	id INTEGER NOT NULL, 
	domain VARCHAR(256) NOT NULL, 
	expires_at DATETIME, 
	registrar VARCHAR(255), 
	auto_renew BOOLEAN, 
	source VARCHAR(32), 
	checked_at DATETIME, 
	PRIMARY KEY (id)
);
CREATE UNIQUE INDEX ix_domain_registrations_domain ON domain_registrations (domain);
CREATE TABLE jobs (
	id VARCHAR(36) NOT NULL, 
	kind VARCHAR(80) NOT NULL, 
	status VARCHAR(20) NOT NULL, 
	payload TEXT, 
	result TEXT, 
	error_message TEXT, 
	attempts INTEGER NOT NULL, 
	max_attempts INTEGER NOT NULL, 
	priority INTEGER, 
	owner_type VARCHAR(40), 
	owner_id VARCHAR(64), 
	scheduled_job_id INTEGER, 
	correlation_id VARCHAR(64), 
	queue_message_id VARCHAR(36), 
	scheduled_at DATETIME, 
	created_at DATETIME, 
	started_at DATETIME, 
	completed_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id)
);
CREATE INDEX ix_jobs_kind ON jobs (kind);
CREATE INDEX ix_jobs_owner_type ON jobs (owner_type);
CREATE INDEX ix_jobs_owner_id ON jobs (owner_id);
CREATE INDEX ix_jobs_created_at ON jobs (created_at);
CREATE INDEX ix_jobs_scheduled_job_id ON jobs (scheduled_job_id);
CREATE INDEX ix_jobs_correlation_id ON jobs (correlation_id);
CREATE INDEX ix_jobs_status ON jobs (status);
CREATE TABLE scheduled_jobs (
	id INTEGER NOT NULL, 
	name VARCHAR(80) NOT NULL, 
	kind VARCHAR(80) NOT NULL, 
	schedule_kind VARCHAR(20) NOT NULL, 
	interval_seconds INTEGER, 
	cron VARCHAR(120), 
	payload TEXT, 
	max_attempts INTEGER NOT NULL, 
	enabled BOOLEAN NOT NULL, 
	next_run_at DATETIME, 
	last_run_at DATETIME, 
	last_job_id VARCHAR(36), 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	UNIQUE (name)
);
CREATE INDEX ix_scheduled_jobs_next_run_at ON scheduled_jobs (next_run_at);
CREATE TABLE notification_preferences (
	id INTEGER NOT NULL, 
	user_id INTEGER NOT NULL, 
	enabled BOOLEAN, 
	channels TEXT, 
	severities TEXT, 
	email VARCHAR(255), 
	discord_webhook VARCHAR(512), 
	telegram_chat_id VARCHAR(64), 
	categories TEXT, 
	quiet_hours_enabled BOOLEAN, 
	quiet_hours_start VARCHAR(5), 
	quiet_hours_end VARCHAR(5), 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	UNIQUE (user_id), 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE TABLE shared_variables (
	id INTEGER NOT NULL, 
	group_id INTEGER NOT NULL, 
	"key" VARCHAR(255) NOT NULL, 
	encrypted_value TEXT NOT NULL, 
	is_secret BOOLEAN, 
	target_service VARCHAR(120), 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_shared_var_key UNIQUE (group_id, "key"), 
	FOREIGN KEY(group_id) REFERENCES shared_variable_groups (id)
);
CREATE TABLE shared_variable_group_attachments (
	id INTEGER NOT NULL, 
	group_id INTEGER NOT NULL, 
	resource_type VARCHAR(50) NOT NULL, 
	resource_id VARCHAR(255) NOT NULL, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_group_attachment UNIQUE (group_id, resource_type, resource_id), 
	FOREIGN KEY(group_id) REFERENCES shared_variable_groups (id)
);
CREATE INDEX ix_group_attachment_resource ON shared_variable_group_attachments (resource_type, resource_id);
CREATE TABLE site_base_domains (
	id INTEGER NOT NULL, 
	domain VARCHAR(253) NOT NULL, 
	is_default BOOLEAN NOT NULL, 
	dns_mode VARCHAR(20) NOT NULL, 
	https_enabled BOOLEAN NOT NULL, 
	dns_provider_config_id INTEGER, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	UNIQUE (domain), 
	FOREIGN KEY(dns_provider_config_id) REFERENCES dns_provider_configs (id) ON DELETE SET NULL
);
CREATE TABLE system_settings (
	id INTEGER NOT NULL, 
	"key" VARCHAR(100) NOT NULL, 
	value TEXT, 
	value_type VARCHAR(20), 
	description VARCHAR(500), 
	updated_at DATETIME, 
	updated_by INTEGER, 
	PRIMARY KEY (id), 
	FOREIGN KEY(updated_by) REFERENCES users (id)
);
CREATE UNIQUE INDEX ix_system_settings_key ON system_settings ("key");
CREATE TABLE audit_logs (
	id INTEGER NOT NULL, 
	action VARCHAR(100) NOT NULL, 
	user_id INTEGER, 
	target_type VARCHAR(50), 
	target_id INTEGER, 
	details TEXT, 
	ip_address VARCHAR(45), 
	user_agent VARCHAR(500), 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE INDEX ix_audit_logs_created_at ON audit_logs (created_at);
CREATE INDEX ix_audit_logs_action ON audit_logs (action);
CREATE TABLE workflows (
	id INTEGER NOT NULL, 
	name VARCHAR(100) NOT NULL, 
	description TEXT, 
	nodes TEXT, 
	edges TEXT, 
	viewport TEXT, 
	is_active BOOLEAN, 
	trigger_type VARCHAR(50), 
	trigger_config TEXT, 
	last_run_at DATETIME, 
	last_status VARCHAR(20), 
	created_at DATETIME, 
	updated_at DATETIME, 
	user_id INTEGER NOT NULL, 
	PRIMARY KEY (id), 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE TABLE agent_rollouts (
	id VARCHAR(36) NOT NULL, 
	version_id VARCHAR(36) NOT NULL, 
	group_id VARCHAR(36), 
	user_id INTEGER, 
	batch_size INTEGER, 
	delay_minutes INTEGER, 
	strategy VARCHAR(20), 
	status VARCHAR(20), 
	total_servers INTEGER, 
	processed_servers INTEGER, 
	failed_servers INTEGER, 
	current_wave INTEGER, 
	server_results JSON, 
	error TEXT, 
	started_at DATETIME, 
	completed_at DATETIME, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(version_id) REFERENCES agent_versions (id), 
	FOREIGN KEY(group_id) REFERENCES server_groups (id), 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE TABLE sanitization_profiles (
	id INTEGER NOT NULL, 
	user_id INTEGER NOT NULL, 
	name VARCHAR(100) NOT NULL, 
	description TEXT, 
	config TEXT NOT NULL, 
	is_default BOOLEAN, 
	is_builtin BOOLEAN, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE TABLE email_domains (
	id INTEGER NOT NULL, 
	name VARCHAR(255) NOT NULL, 
	is_active BOOLEAN, 
	dkim_selector VARCHAR(63), 
	dkim_private_key_path VARCHAR(500), 
	dkim_public_key TEXT, 
	spf_record VARCHAR(500), 
	dmarc_record VARCHAR(500), 
	dns_provider_id INTEGER, 
	dns_zone_id VARCHAR(255), 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(dns_provider_id) REFERENCES dns_provider_configs (id)
);
CREATE UNIQUE INDEX ix_email_domains_name ON email_domains (name);
CREATE TABLE oauth_identities (
	id INTEGER NOT NULL, 
	user_id INTEGER NOT NULL, 
	provider VARCHAR(50) NOT NULL, 
	provider_user_id VARCHAR(256) NOT NULL, 
	provider_email VARCHAR(256), 
	provider_display_name VARCHAR(256), 
	access_token_encrypted TEXT, 
	refresh_token_encrypted TEXT, 
	token_expires_at DATETIME, 
	created_at DATETIME, 
	updated_at DATETIME, 
	last_login_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_provider_identity UNIQUE (provider, provider_user_id), 
	FOREIGN KEY(user_id) REFERENCES users (id) ON DELETE CASCADE
);
CREATE INDEX ix_oauth_identities_user_id ON oauth_identities (user_id);
CREATE TABLE source_connections (
	id INTEGER NOT NULL, 
	user_id INTEGER NOT NULL, 
	provider VARCHAR(40) NOT NULL, 
	provider_account_id VARCHAR(120), 
	provider_username VARCHAR(120), 
	display_name VARCHAR(180), 
	avatar_url VARCHAR(500), 
	access_token_encrypted TEXT NOT NULL, 
	scope VARCHAR(500), 
	created_at DATETIME, 
	updated_at DATETIME, 
	last_used_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_source_connection_user_provider UNIQUE (user_id, provider), 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE INDEX ix_source_connections_created_at ON source_connections (created_at);
CREATE INDEX ix_source_connections_provider_account_id ON source_connections (provider_account_id);
CREATE INDEX ix_source_connections_provider ON source_connections (provider);
CREATE INDEX ix_source_connections_user_id ON source_connections (user_id);
CREATE TABLE registrar_connections (
	id INTEGER NOT NULL, 
	user_id INTEGER, 
	provider VARCHAR(40) NOT NULL, 
	name VARCHAR(120), 
	api_key_encrypted TEXT, 
	api_secret_encrypted TEXT, 
	account_label VARCHAR(180), 
	config_json TEXT, 
	last_synced_at DATETIME, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE TABLE api_keys (
	id INTEGER NOT NULL, 
	user_id INTEGER NOT NULL, 
	name VARCHAR(100) NOT NULL, 
	key_prefix VARCHAR(8) NOT NULL, 
	key_hash VARCHAR(256) NOT NULL, 
	scopes TEXT, 
	tier VARCHAR(20), 
	is_active BOOLEAN, 
	expires_at DATETIME, 
	last_used_at DATETIME, 
	last_used_ip VARCHAR(45), 
	usage_count INTEGER, 
	created_at DATETIME, 
	revoked_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(user_id) REFERENCES users (id), 
	UNIQUE (key_hash)
);
CREATE TABLE event_subscriptions (
	id INTEGER NOT NULL, 
	user_id INTEGER NOT NULL, 
	name VARCHAR(100) NOT NULL, 
	url VARCHAR(2048) NOT NULL, 
	secret VARCHAR(256), 
	events TEXT NOT NULL, 
	is_active BOOLEAN, 
	headers TEXT, 
	retry_count INTEGER, 
	timeout_seconds INTEGER, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE TABLE invitations (
	id INTEGER NOT NULL, 
	email VARCHAR(255), 
	token VARCHAR(64) NOT NULL, 
	role VARCHAR(20) NOT NULL, 
	permissions TEXT, 
	invited_by INTEGER NOT NULL, 
	expires_at DATETIME, 
	accepted_at DATETIME, 
	accepted_by INTEGER, 
	status VARCHAR(20) NOT NULL, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(invited_by) REFERENCES users (id), 
	FOREIGN KEY(accepted_by) REFERENCES users (id)
);
CREATE INDEX ix_invitations_status ON invitations (status);
CREATE UNIQUE INDEX ix_invitations_token ON invitations (token);
CREATE TABLE server_templates (
	id INTEGER NOT NULL, 
	name VARCHAR(128) NOT NULL, 
	description TEXT, 
	category VARCHAR(64), 
	version INTEGER, 
	parent_id INTEGER, 
	packages_json TEXT, 
	services_json TEXT, 
	firewall_rules_json TEXT, 
	files_json TEXT, 
	users_json TEXT, 
	sysctl_json TEXT, 
	auto_remediate BOOLEAN, 
	remediation_approval_required BOOLEAN, 
	created_by INTEGER, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	UNIQUE (name), 
	FOREIGN KEY(parent_id) REFERENCES server_templates (id), 
	FOREIGN KEY(created_by) REFERENCES users (id)
);
CREATE TABLE workspaces (
	id INTEGER NOT NULL, 
	name VARCHAR(128) NOT NULL, 
	slug VARCHAR(128) NOT NULL, 
	description TEXT, 
	logo_url VARCHAR(512), 
	primary_color VARCHAR(7), 
	settings_json TEXT, 
	max_servers INTEGER, 
	max_users INTEGER, 
	max_api_calls INTEGER, 
	status VARCHAR(32), 
	billing_notes TEXT, 
	created_by INTEGER, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	UNIQUE (name), 
	UNIQUE (slug), 
	FOREIGN KEY(created_by) REFERENCES users (id)
);
CREATE TABLE resource_grants (
	id INTEGER NOT NULL, 
	user_id INTEGER NOT NULL, 
	resource_type VARCHAR(32) NOT NULL, 
	resource_id INTEGER NOT NULL, 
	role VARCHAR(16), 
	granted_by INTEGER, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_resource_grant UNIQUE (user_id, resource_type, resource_id), 
	FOREIGN KEY(user_id) REFERENCES users (id), 
	FOREIGN KEY(granted_by) REFERENCES users (id)
);
CREATE INDEX ix_resource_grants_resource_id ON resource_grants (resource_id);
CREATE INDEX ix_resource_grants_user_id ON resource_grants (user_id);
CREATE TABLE dns_zones (
	id INTEGER NOT NULL, 
	domain VARCHAR(256) NOT NULL, 
	provider VARCHAR(64), 
	provider_zone_id VARCHAR(128), 
	provider_config_json TEXT, 
	dns_provider_config_id INTEGER, 
	status VARCHAR(32), 
	last_sync_at DATETIME, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	UNIQUE (domain), 
	FOREIGN KEY(dns_provider_config_id) REFERENCES dns_provider_configs (id)
);
CREATE TABLE dns_changes (
	id INTEGER NOT NULL, 
	dns_provider_config_id INTEGER, 
	provider VARCHAR(64) NOT NULL, 
	provider_zone_id VARCHAR(128), 
	action VARCHAR(16) NOT NULL, 
	record_type VARCHAR(10), 
	name VARCHAR(256), 
	content TEXT, 
	provider_record_id VARCHAR(128), 
	source VARCHAR(40), 
	result VARCHAR(16) NOT NULL, 
	error TEXT, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(dns_provider_config_id) REFERENCES dns_provider_configs (id)
);
CREATE INDEX ix_dns_changes_provider_zone_id ON dns_changes (provider_zone_id);
CREATE INDEX ix_dns_changes_created_at ON dns_changes (created_at);
CREATE INDEX ix_dns_changes_dns_provider_config_id ON dns_changes (dns_provider_config_id);
CREATE TABLE cloud_providers (
	id INTEGER NOT NULL, 
	name VARCHAR(64) NOT NULL, 
	provider_type VARCHAR(32) NOT NULL, 
	api_key_encrypted TEXT, 
	is_active BOOLEAN, 
	created_by INTEGER, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(created_by) REFERENCES users (id)
);
CREATE TABLE installed_plugins (
	id INTEGER NOT NULL, 
	name VARCHAR(128) NOT NULL, 
	display_name VARCHAR(256) NOT NULL, 
	slug VARCHAR(128) NOT NULL, 
	version VARCHAR(32) NOT NULL, 
	description TEXT, 
	author VARCHAR(128), 
	homepage VARCHAR(512), 
	repository VARCHAR(512), 
	license VARCHAR(64), 
	category VARCHAR(64), 
	source_url VARCHAR(1024), 
	source_type VARCHAR(32), 
	backend_path VARCHAR(512), 
	frontend_path VARCHAR(512), 
	entry_point VARCHAR(256), 
	url_prefix VARCHAR(256), 
	frontend_entry VARCHAR(256), 
	manifest_json TEXT, 
	config_json TEXT, 
	status VARCHAR(32), 
	error_message TEXT, 
	has_frontend BOOLEAN, 
	has_backend BOOLEAN, 
	installed_by INTEGER, 
	installed_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	UNIQUE (name), 
	UNIQUE (slug), 
	FOREIGN KEY(installed_by) REFERENCES users (id)
);
CREATE TABLE ai_conversations (
	id VARCHAR(64) NOT NULL, 
	user_id INTEGER NOT NULL, 
	title VARCHAR(256), 
	mode VARCHAR(16), 
	model_name VARCHAR(128), 
	export_json TEXT, 
	last_page VARCHAR(256), 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE INDEX ix_ai_conversations_user_id ON ai_conversations (user_id);
CREATE TABLE passkey_credentials (
	id INTEGER NOT NULL, 
	user_id INTEGER NOT NULL, 
	credential_id VARCHAR(500) NOT NULL, 
	public_key TEXT NOT NULL, 
	sign_count INTEGER, 
	transports TEXT, 
	device_name VARCHAR(200), 
	is_active BOOLEAN, 
	created_at DATETIME, 
	last_used_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE INDEX ix_passkey_credentials_user_id ON passkey_credentials (user_id);
CREATE UNIQUE INDEX ix_passkey_credentials_credential_id ON passkey_credentials (credential_id);
CREATE TABLE cloudflare_workers (
	id INTEGER NOT NULL, 
	name VARCHAR(128) NOT NULL, 
	account_id VARCHAR(64) NOT NULL, 
	dns_provider_config_id INTEGER, 
	source TEXT, 
	compatibility_date VARCHAR(20), 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_cf_worker_account_name UNIQUE (account_id, name), 
	FOREIGN KEY(dns_provider_config_id) REFERENCES dns_provider_configs (id)
);
CREATE INDEX ix_cloudflare_workers_name ON cloudflare_workers (name);
CREATE TABLE cloudflare_tunnels (
	id INTEGER NOT NULL, 
	tunnel_id VARCHAR(64) NOT NULL, 
	name VARCHAR(128) NOT NULL, 
	account_id VARCHAR(64) NOT NULL, 
	dns_provider_config_id INTEGER, 
	token_encrypted TEXT, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_cf_tunnel_account_id UNIQUE (account_id, tunnel_id), 
	FOREIGN KEY(dns_provider_config_id) REFERENCES dns_provider_configs (id)
);
CREATE INDEX ix_cloudflare_tunnels_tunnel_id ON cloudflare_tunnels (tunnel_id);
CREATE TABLE backup_runs (
	id INTEGER NOT NULL, 
	policy_id INTEGER NOT NULL, 
	job_id VARCHAR(36), 
	kind VARCHAR(20) NOT NULL, 
	status VARCHAR(20) NOT NULL, 
	started_at DATETIME, 
	finished_at DATETIME, 
	duration_seconds INTEGER, 
	size_local BIGINT, 
	size_remote BIGINT, 
	cost_local NUMERIC(10, 4), 
	cost_remote NUMERIC(10, 4), 
	storage_path TEXT, 
	remote_key TEXT, 
	verified BOOLEAN, 
	error_message TEXT, 
	metadata_json TEXT, 
	PRIMARY KEY (id), 
	FOREIGN KEY(policy_id) REFERENCES backup_policies (id)
);
CREATE INDEX ix_backup_runs_job_id ON backup_runs (job_id);
CREATE INDEX ix_backup_runs_policy_id ON backup_runs (policy_id);
CREATE TABLE queues (
	id VARCHAR(36) NOT NULL, 
	group_id VARCHAR(36) NOT NULL, 
	slug VARCHAR(128) NOT NULL, 
	name VARCHAR(256) NOT NULL, 
	description TEXT, 
	config_json TEXT, 
	created_at DATETIME NOT NULL, 
	updated_at DATETIME NOT NULL, 
	PRIMARY KEY (id), 
	CONSTRAINT uix_queue_group_slug UNIQUE (group_id, slug), 
	FOREIGN KEY(group_id) REFERENCES queue_groups (id)
);
CREATE INDEX ix_queues_slug ON queues (slug);
CREATE INDEX ix_queues_group_id ON queues (group_id);
CREATE TABLE notification_deliveries (
	id INTEGER NOT NULL, 
	notification_id INTEGER NOT NULL, 
	recipient_user_id INTEGER, 
	channel VARCHAR(40) NOT NULL, 
	target VARCHAR(512), 
	status VARCHAR(20), 
	attempts INTEGER, 
	error TEXT, 
	provider_message_id VARCHAR(255), 
	created_at DATETIME, 
	sent_at DATETIME, 
	read_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(notification_id) REFERENCES notifications (id), 
	FOREIGN KEY(recipient_user_id) REFERENCES users (id)
);
CREATE INDEX ix_notification_deliveries_status ON notification_deliveries (status);
CREATE INDEX ix_notification_deliveries_notification_id ON notification_deliveries (notification_id);
CREATE INDEX ix_notification_deliveries_channel ON notification_deliveries (channel);
CREATE INDEX ix_notification_deliveries_created_at ON notification_deliveries (created_at);
CREATE INDEX ix_notification_deliveries_recipient_user_id ON notification_deliveries (recipient_user_id);
CREATE TABLE email_provider_connections (
	id INTEGER NOT NULL, 
	provider VARCHAR(40) NOT NULL, 
	name VARCHAR(120) NOT NULL, 
	credentials_json TEXT, 
	from_address VARCHAR(255), 
	from_name VARCHAR(120), 
	is_default BOOLEAN, 
	is_active BOOLEAN, 
	uses_notifications BOOLEAN NOT NULL, 
	uses_relay BOOLEAN NOT NULL, 
	relay_priority INTEGER NOT NULL, 
	created_by INTEGER, 
	created_at DATETIME, 
	updated_at DATETIME, 
	last_tested_at DATETIME, 
	last_test_ok BOOLEAN, 
	PRIMARY KEY (id), 
	FOREIGN KEY(created_by) REFERENCES users (id)
);
CREATE INDEX ix_email_provider_connections_is_default ON email_provider_connections (is_default);
CREATE TABLE projects (
	id INTEGER NOT NULL, 
	workspace_id INTEGER NOT NULL, 
	name VARCHAR(128) NOT NULL, 
	slug VARCHAR(128) NOT NULL, 
	description TEXT, 
	metadata_json TEXT, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_project_workspace_slug UNIQUE (workspace_id, slug), 
	FOREIGN KEY(workspace_id) REFERENCES workspaces (id)
);
CREATE INDEX ix_projects_workspace_id ON projects (workspace_id);
CREATE TABLE workflow_executions (
	id INTEGER NOT NULL, 
	workflow_id INTEGER NOT NULL, 
	status VARCHAR(20), 
	trigger_type VARCHAR(50), 
	context TEXT, 
	results TEXT, 
	started_at DATETIME, 
	completed_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(workflow_id) REFERENCES workflows (id)
);
CREATE TABLE servers (
	id VARCHAR(36) NOT NULL, 
	name VARCHAR(100) NOT NULL, 
	description TEXT, 
	hostname VARCHAR(255), 
	ip_address VARCHAR(45), 
	group_id VARCHAR(36), 
	workspace_id INTEGER, 
	tags JSON, 
	status VARCHAR(20), 
	last_seen DATETIME, 
	last_error TEXT, 
	onboarding_state VARCHAR(40), 
	onboarding_progress TEXT, 
	onboarding_updated_at DATETIME, 
	agent_version VARCHAR(20), 
	agent_id VARCHAR(36), 
	auto_upgrade BOOLEAN, 
	upgrade_channel VARCHAR(20), 
	os_type VARCHAR(20), 
	os_version VARCHAR(100), 
	platform VARCHAR(100), 
	architecture VARCHAR(20), 
	cpu_cores INTEGER, 
	cpu_model VARCHAR(200), 
	total_memory BIGINT, 
	total_disk BIGINT, 
	docker_version VARCHAR(50), 
	agent_install_dir VARCHAR(255), 
	agent_config_dir VARCHAR(255), 
	api_key_hash VARCHAR(256), 
	api_key_prefix VARCHAR(12), 
	api_secret_encrypted TEXT, 
	permissions JSON, 
	allowed_ips JSON, 
	api_key_pending_hash VARCHAR(256), 
	api_key_pending_prefix VARCHAR(12), 
	api_secret_pending_encrypted TEXT, 
	api_key_rotation_expires DATETIME, 
	api_key_rotation_id VARCHAR(36), 
	api_key_last_rotated DATETIME, 
	registration_token_hash VARCHAR(256), 
	registration_token_expires DATETIME, 
	registered_at DATETIME, 
	registered_by INTEGER, 
	cached_capabilities JSON, 
	cached_runtimes JSON, 
	cached_runtime_managers JSON, 
	cached_allowed_paths JSON, 
	cached_sudo VARCHAR(20), 
	cached_systemd_json BOOLEAN, 
	capabilities_at DATETIME, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(group_id) REFERENCES server_groups (id), 
	FOREIGN KEY(workspace_id) REFERENCES workspaces (id), 
	FOREIGN KEY(registered_by) REFERENCES users (id)
);
CREATE INDEX ix_servers_status ON servers (status);
CREATE INDEX ix_servers_workspace_id ON servers (workspace_id);
CREATE INDEX ix_servers_group_id ON servers (group_id);
CREATE UNIQUE INDEX ix_servers_agent_id ON servers (agent_id);
CREATE TABLE email_accounts (
	id INTEGER NOT NULL, 
	email VARCHAR(255) NOT NULL, 
	username VARCHAR(255) NOT NULL, 
	password_hash VARCHAR(500) NOT NULL, 
	domain_id INTEGER NOT NULL, 
	quota_mb INTEGER, 
	quota_used_mb INTEGER, 
	is_active BOOLEAN, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(domain_id) REFERENCES email_domains (id)
);
CREATE UNIQUE INDEX ix_email_accounts_email ON email_accounts (email);
CREATE TABLE email_aliases (
	id INTEGER NOT NULL, 
	source VARCHAR(255) NOT NULL, 
	destination VARCHAR(255) NOT NULL, 
	domain_id INTEGER NOT NULL, 
	is_active BOOLEAN, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(domain_id) REFERENCES email_domains (id)
);
CREATE INDEX ix_email_aliases_source ON email_aliases (source);
CREATE TABLE container_registries (
	id INTEGER NOT NULL, 
	name VARCHAR(180) NOT NULL, 
	provider VARCHAR(40) NOT NULL, 
	registry_url VARCHAR(255), 
	username VARCHAR(180), 
	secret_encrypted TEXT, 
	workspace_id INTEGER, 
	created_by INTEGER, 
	last_used_at DATETIME, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(workspace_id) REFERENCES workspaces (id), 
	FOREIGN KEY(created_by) REFERENCES users (id)
);
CREATE INDEX ix_container_registries_created_at ON container_registries (created_at);
CREATE INDEX ix_container_registries_provider ON container_registries (provider);
CREATE INDEX ix_container_registries_workspace_id ON container_registries (workspace_id);
CREATE TABLE api_usage_logs (
	id INTEGER NOT NULL, 
	api_key_id INTEGER, 
	user_id INTEGER, 
	method VARCHAR(10) NOT NULL, 
	endpoint VARCHAR(500) NOT NULL, 
	blueprint VARCHAR(100), 
	status_code INTEGER NOT NULL, 
	response_time_ms FLOAT, 
	ip_address VARCHAR(45), 
	user_agent VARCHAR(500), 
	request_size INTEGER, 
	response_size INTEGER, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(api_key_id) REFERENCES api_keys (id), 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE INDEX ix_api_usage_logs_created_at ON api_usage_logs (created_at);
CREATE TABLE api_usage_summaries (
	id INTEGER NOT NULL, 
	period_start DATETIME NOT NULL, 
	api_key_id INTEGER, 
	user_id INTEGER, 
	endpoint VARCHAR(500), 
	total_requests INTEGER, 
	success_count INTEGER, 
	client_error_count INTEGER, 
	server_error_count INTEGER, 
	avg_response_time_ms FLOAT, 
	max_response_time_ms FLOAT, 
	PRIMARY KEY (id), 
	FOREIGN KEY(api_key_id) REFERENCES api_keys (id), 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE INDEX ix_api_usage_summaries_period_start ON api_usage_summaries (period_start);
CREATE TABLE event_deliveries (
	id INTEGER NOT NULL, 
	subscription_id INTEGER NOT NULL, 
	event_type VARCHAR(100) NOT NULL, 
	payload TEXT, 
	status VARCHAR(20), 
	http_status INTEGER, 
	response_body VARCHAR(1000), 
	attempts INTEGER, 
	next_retry_at DATETIME, 
	delivered_at DATETIME, 
	created_at DATETIME, 
	duration_ms FLOAT, 
	correlation_id VARCHAR(64), 
	PRIMARY KEY (id), 
	FOREIGN KEY(subscription_id) REFERENCES event_subscriptions (id)
);
CREATE INDEX ix_event_deliveries_created_at ON event_deliveries (created_at);
CREATE INDEX ix_event_deliveries_correlation_id ON event_deliveries (correlation_id);
CREATE TABLE workspace_members (
	id INTEGER NOT NULL, 
	workspace_id INTEGER NOT NULL, 
	user_id INTEGER NOT NULL, 
	role VARCHAR(32), 
	joined_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_workspace_user UNIQUE (workspace_id, user_id), 
	FOREIGN KEY(workspace_id) REFERENCES workspaces (id), 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE TABLE workspace_api_keys (
	id INTEGER NOT NULL, 
	workspace_id INTEGER NOT NULL, 
	name VARCHAR(128) NOT NULL, 
	key_hash VARCHAR(256) NOT NULL, 
	key_prefix VARCHAR(16), 
	scopes_json TEXT, 
	is_active BOOLEAN, 
	expires_at DATETIME, 
	created_by INTEGER, 
	created_at DATETIME, 
	last_used_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(workspace_id) REFERENCES workspaces (id), 
	FOREIGN KEY(created_by) REFERENCES users (id)
);
CREATE TABLE dns_records (
	id INTEGER NOT NULL, 
	zone_id INTEGER NOT NULL, 
	record_type VARCHAR(10) NOT NULL, 
	name VARCHAR(256) NOT NULL, 
	content TEXT NOT NULL, 
	ttl INTEGER, 
	priority INTEGER, 
	proxied BOOLEAN, 
	provider_record_id VARCHAR(128), 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(zone_id) REFERENCES dns_zones (id)
);
CREATE TABLE cloud_servers (
	id INTEGER NOT NULL, 
	provider_id INTEGER NOT NULL, 
	external_id VARCHAR(128), 
	name VARCHAR(128) NOT NULL, 
	hostname VARCHAR(256), 
	region VARCHAR(64), 
	size VARCHAR(64), 
	image VARCHAR(128), 
	ip_address VARCHAR(45), 
	ipv6_address VARCHAR(64), 
	status VARCHAR(32), 
	monthly_cost FLOAT, 
	currency VARCHAR(3), 
	agent_installed BOOLEAN, 
	ssh_key_id VARCHAR(128), 
	metadata_json TEXT, 
	created_by INTEGER, 
	created_at DATETIME, 
	destroyed_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(provider_id) REFERENCES cloud_providers (id), 
	FOREIGN KEY(created_by) REFERENCES users (id)
);
CREATE TABLE ai_messages (
	id INTEGER NOT NULL, 
	conversation_id VARCHAR(64) NOT NULL, 
	role VARCHAR(16) NOT NULL, 
	content TEXT, 
	tool_calls_json TEXT, 
	usage_json TEXT, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(conversation_id) REFERENCES ai_conversations (id) ON DELETE CASCADE
);
CREATE INDEX ix_ai_messages_conversation_id ON ai_messages (conversation_id);
CREATE TABLE ai_pending_actions (
	id VARCHAR(64) NOT NULL, 
	conversation_id VARCHAR(64) NOT NULL, 
	user_id INTEGER NOT NULL, 
	tool_name VARCHAR(128) NOT NULL, 
	plugin_slug VARCHAR(128), 
	params_json TEXT, 
	summary TEXT, 
	status VARCHAR(16), 
	result_json TEXT, 
	created_at DATETIME, 
	expires_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(conversation_id) REFERENCES ai_conversations (id) ON DELETE CASCADE, 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE INDEX ix_ai_pending_actions_user_id ON ai_pending_actions (user_id);
CREATE INDEX ix_ai_pending_actions_conversation_id ON ai_pending_actions (conversation_id);
CREATE TABLE secret_vaults (
	id INTEGER NOT NULL, 
	name VARCHAR(200) NOT NULL, 
	slug VARCHAR(220) NOT NULL, 
	description TEXT, 
	created_by INTEGER, 
	workspace_id INTEGER, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	UNIQUE (name), 
	FOREIGN KEY(created_by) REFERENCES users (id), 
	FOREIGN KEY(workspace_id) REFERENCES workspaces (id)
);
CREATE UNIQUE INDEX ix_secret_vaults_slug ON secret_vaults (slug);
CREATE INDEX ix_secret_vaults_workspace_id ON secret_vaults (workspace_id);
CREATE TABLE webhook_endpoints (
	id INTEGER NOT NULL, 
	name VARCHAR(200) NOT NULL, 
	slug VARCHAR(220) NOT NULL, 
	secret VARCHAR(500) NOT NULL, 
	is_active BOOLEAN, 
	filter_paths TEXT, 
	forward_url VARCHAR(500), 
	retry_count INTEGER, 
	created_by INTEGER, 
	workspace_id INTEGER, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	UNIQUE (name), 
	FOREIGN KEY(created_by) REFERENCES users (id), 
	FOREIGN KEY(workspace_id) REFERENCES workspaces (id)
);
CREATE UNIQUE INDEX ix_webhook_endpoints_slug ON webhook_endpoints (slug);
CREATE INDEX ix_webhook_endpoints_workspace_id ON webhook_endpoints (workspace_id);
CREATE TABLE queue_messages (
	id VARCHAR(36) NOT NULL, 
	queue_id VARCHAR(36) NOT NULL, 
	group_id VARCHAR(36) NOT NULL, 
	status VARCHAR(32) NOT NULL, 
	priority INTEGER NOT NULL, 
	payload_json TEXT NOT NULL, 
	result_json TEXT, 
	error_message TEXT, 
	attempts INTEGER NOT NULL, 
	max_attempts INTEGER NOT NULL, 
	visible_after DATETIME NOT NULL, 
	invisible_until DATETIME, 
	created_at DATETIME NOT NULL, 
	updated_at DATETIME NOT NULL, 
	completed_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(queue_id) REFERENCES queues (id), 
	FOREIGN KEY(group_id) REFERENCES queue_groups (id)
);
CREATE INDEX ix_queue_messages_invisible_until ON queue_messages (invisible_until);
CREATE INDEX ix_queue_messages_priority ON queue_messages (priority);
CREATE INDEX ix_queue_messages_group_id ON queue_messages (group_id);
CREATE INDEX ix_queue_messages_created_at ON queue_messages (created_at);
CREATE INDEX ix_queue_messages_queue_id ON queue_messages (queue_id);
CREATE INDEX ix_queue_messages_visible_after ON queue_messages (visible_after);
CREATE INDEX ix_queue_messages_status ON queue_messages (status);
CREATE TABLE system_events (
	id INTEGER NOT NULL, 
	timestamp DATETIME NOT NULL, 
	source VARCHAR(50) NOT NULL, 
	event_type VARCHAR(100) NOT NULL, 
	severity VARCHAR(20) NOT NULL, 
	resource_type VARCHAR(50), 
	resource_id VARCHAR(100), 
	actor_user_id INTEGER, 
	workspace_id INTEGER, 
	message TEXT, 
	correlation_id VARCHAR(64), 
	payload_json TEXT, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(actor_user_id) REFERENCES users (id), 
	FOREIGN KEY(workspace_id) REFERENCES workspaces (id)
);
CREATE INDEX ix_system_events_severity ON system_events (severity);
CREATE INDEX ix_system_events_resource_id ON system_events (resource_id);
CREATE INDEX ix_system_events_workspace_id ON system_events (workspace_id);
CREATE INDEX ix_system_events_correlation_id ON system_events (correlation_id);
CREATE INDEX idx_system_events_resource ON system_events (resource_type, resource_id);
CREATE INDEX idx_system_events_severity_timestamp ON system_events (severity, timestamp);
CREATE INDEX ix_system_events_source ON system_events (source);
CREATE INDEX ix_system_events_actor_user_id ON system_events (actor_user_id);
CREATE INDEX ix_system_events_created_at ON system_events (created_at);
CREATE INDEX ix_system_events_event_type ON system_events (event_type);
CREATE INDEX idx_system_events_source_timestamp ON system_events (source, timestamp);
CREATE INDEX ix_system_events_resource_type ON system_events (resource_type);
CREATE INDEX idx_system_events_type_timestamp ON system_events (event_type, timestamp);
CREATE INDEX ix_system_events_timestamp ON system_events (timestamp);
CREATE TABLE ddns_hosts (
	id INTEGER NOT NULL, 
	zone_id INTEGER NOT NULL, 
	record_name VARCHAR(256) NOT NULL, 
	label VARCHAR(128), 
	token VARCHAR(64) NOT NULL, 
	last_ip VARCHAR(45), 
	last_update_at DATETIME, 
	enabled BOOLEAN NOT NULL, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(zone_id) REFERENCES dns_zones (id)
);
CREATE UNIQUE INDEX ix_ddns_hosts_token ON ddns_hosts (token);
CREATE TABLE environments (
	id INTEGER NOT NULL, 
	project_id INTEGER NOT NULL, 
	name VARCHAR(64) NOT NULL, 
	slug VARCHAR(64) NOT NULL, 
	is_default BOOLEAN, 
	"order" INTEGER, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_environment_project_slug UNIQUE (project_id, slug), 
	FOREIGN KEY(project_id) REFERENCES projects (id)
);
CREATE INDEX ix_environments_project_id ON environments (project_id);
CREATE TABLE proxy_stacks (
	id VARCHAR(36) NOT NULL, 
	server_id VARCHAR(36) NOT NULL, 
	proxy_type VARCHAR(20) NOT NULL, 
	status VARCHAR(20) NOT NULL, 
	compose_path VARCHAR(512), 
	networks TEXT, 
	custom_snippet TEXT, 
	last_regenerated_at DATETIME, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(server_id) REFERENCES servers (id)
);
CREATE UNIQUE INDEX ix_proxy_stacks_server_id ON proxy_stacks (server_id);
CREATE TABLE workflow_logs (
	id INTEGER NOT NULL, 
	execution_id INTEGER NOT NULL, 
	level VARCHAR(10), 
	message TEXT NOT NULL, 
	node_id VARCHAR(100), 
	timestamp DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(execution_id) REFERENCES workflow_executions (id)
);
CREATE TABLE server_metrics (
	id INTEGER NOT NULL, 
	server_id VARCHAR(36) NOT NULL, 
	timestamp DATETIME, 
	cpu_percent FLOAT, 
	memory_percent FLOAT, 
	memory_used BIGINT, 
	disk_percent FLOAT, 
	disk_used BIGINT, 
	network_rx BIGINT, 
	network_tx BIGINT, 
	network_rx_rate FLOAT, 
	network_tx_rate FLOAT, 
	container_count INTEGER, 
	container_running INTEGER, 
	extra JSON, 
	PRIMARY KEY (id), 
	FOREIGN KEY(server_id) REFERENCES servers (id)
);
CREATE INDEX ix_server_metrics_server_time ON server_metrics (server_id, timestamp);
CREATE INDEX ix_server_metrics_server_id ON server_metrics (server_id);
CREATE INDEX ix_server_metrics_timestamp ON server_metrics (timestamp);
CREATE TABLE server_commands (
	id VARCHAR(36) NOT NULL, 
	server_id VARCHAR(36) NOT NULL, 
	user_id INTEGER, 
	command_type VARCHAR(50), 
	command_data JSON, 
	status VARCHAR(20), 
	started_at DATETIME, 
	completed_at DATETIME, 
	result JSON, 
	error TEXT, 
	exit_code INTEGER, 
	retry_count INTEGER, 
	max_retries INTEGER, 
	next_retry_at DATETIME, 
	backoff_seconds INTEGER, 
	queued BOOLEAN, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(server_id) REFERENCES servers (id), 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE INDEX ix_server_commands_server_id ON server_commands (server_id);
CREATE TABLE agent_sessions (
	id VARCHAR(36) NOT NULL, 
	server_id VARCHAR(36) NOT NULL, 
	session_token VARCHAR(256), 
	socket_id VARCHAR(100), 
	connected_at DATETIME, 
	last_heartbeat DATETIME, 
	ip_address VARCHAR(45), 
	user_agent VARCHAR(255), 
	heartbeat_latency_ms FLOAT, 
	avg_latency_ms FLOAT, 
	latency_samples INTEGER, 
	is_active BOOLEAN, 
	disconnected_at DATETIME, 
	disconnect_reason VARCHAR(100), 
	PRIMARY KEY (id), 
	FOREIGN KEY(server_id) REFERENCES servers (id)
);
CREATE INDEX ix_agent_sessions_server_id ON agent_sessions (server_id);
CREATE INDEX ix_agent_sessions_is_active ON agent_sessions (is_active);
CREATE TABLE security_alerts (
	id VARCHAR(36) NOT NULL, 
	server_id VARCHAR(36), 
	alert_type VARCHAR(50) NOT NULL, 
	severity VARCHAR(20) NOT NULL, 
	source_ip VARCHAR(45), 
	details JSON, 
	status VARCHAR(20), 
	created_at DATETIME, 
	acknowledged_at DATETIME, 
	acknowledged_by INTEGER, 
	resolved_at DATETIME, 
	resolved_by INTEGER, 
	PRIMARY KEY (id), 
	FOREIGN KEY(server_id) REFERENCES servers (id), 
	FOREIGN KEY(acknowledged_by) REFERENCES users (id), 
	FOREIGN KEY(resolved_by) REFERENCES users (id)
);
CREATE INDEX ix_security_alerts_status ON security_alerts (status);
CREATE INDEX ix_security_alerts_server_id ON security_alerts (server_id);
CREATE INDEX ix_security_alerts_created_at ON security_alerts (created_at);
CREATE INDEX ix_security_alerts_alert_type ON security_alerts (alert_type);
CREATE INDEX ix_security_alerts_severity ON security_alerts (severity);
CREATE TABLE email_forwarding_rules (
	id INTEGER NOT NULL, 
	account_id INTEGER NOT NULL, 
	destination VARCHAR(255) NOT NULL, 
	keep_copy BOOLEAN, 
	is_active BOOLEAN, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(account_id) REFERENCES email_accounts (id)
);
CREATE TABLE server_alert_thresholds (
	id VARCHAR(36) NOT NULL, 
	server_id VARCHAR(36), 
	metric VARCHAR(20) NOT NULL, 
	warning_threshold FLOAT, 
	critical_threshold FLOAT, 
	duration_seconds INTEGER, 
	enabled BOOLEAN, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(server_id) REFERENCES servers (id)
);
CREATE INDEX ix_server_alert_thresholds_server_id ON server_alert_thresholds (server_id);
CREATE TABLE metric_alerts (
	id VARCHAR(36) NOT NULL, 
	server_id VARCHAR(36) NOT NULL, 
	metric VARCHAR(20) NOT NULL, 
	severity VARCHAR(10) NOT NULL, 
	value FLOAT, 
	threshold FLOAT, 
	duration_seconds INTEGER, 
	status VARCHAR(20), 
	acknowledged_by INTEGER, 
	created_at DATETIME, 
	resolved_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(server_id) REFERENCES servers (id), 
	FOREIGN KEY(acknowledged_by) REFERENCES users (id)
);
CREATE INDEX ix_metric_alerts_status ON metric_alerts (status);
CREATE INDEX ix_metric_alerts_server_id ON metric_alerts (server_id);
CREATE TABLE agent_plugin_installs (
	id INTEGER NOT NULL, 
	plugin_id INTEGER NOT NULL, 
	server_id INTEGER NOT NULL, 
	status VARCHAR(32), 
	installed_version VARCHAR(32), 
	config_json TEXT, 
	error_message TEXT, 
	installed_at DATETIME, 
	updated_at DATETIME, 
	last_health_check DATETIME, 
	health_status VARCHAR(32), 
	metrics_json TEXT, 
	PRIMARY KEY (id), 
	FOREIGN KEY(plugin_id) REFERENCES agent_plugins (id), 
	FOREIGN KEY(server_id) REFERENCES servers (id)
);
CREATE TABLE server_template_assignments (
	id INTEGER NOT NULL, 
	template_id INTEGER NOT NULL, 
	server_id INTEGER NOT NULL, 
	status VARCHAR(32), 
	drift_report_json TEXT, 
	last_check_at DATETIME, 
	last_remediation_at DATETIME, 
	applied_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_template_server UNIQUE (template_id, server_id), 
	FOREIGN KEY(template_id) REFERENCES server_templates (id), 
	FOREIGN KEY(server_id) REFERENCES servers (id)
);
CREATE TABLE tunnels (
	id VARCHAR(36) NOT NULL, 
	name VARCHAR(120), 
	edge_server_id VARCHAR(36) NOT NULL, 
	private_server_id VARCHAR(36) NOT NULL, 
	interface_name VARCHAR(15) NOT NULL, 
	subnet VARCHAR(43) NOT NULL, 
	edge_wg_ip VARCHAR(45) NOT NULL, 
	private_wg_ip VARCHAR(45) NOT NULL, 
	listen_port INTEGER, 
	edge_pubkey VARCHAR(64), 
	private_pubkey VARCHAR(64), 
	status VARCHAR(20), 
	last_handshake_at DATETIME, 
	last_error TEXT, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(edge_server_id) REFERENCES servers (id), 
	FOREIGN KEY(private_server_id) REFERENCES servers (id)
);
CREATE INDEX ix_tunnels_edge_server_id ON tunnels (edge_server_id);
CREATE INDEX ix_tunnels_status ON tunnels (status);
CREATE INDEX ix_tunnels_private_server_id ON tunnels (private_server_id);
CREATE TABLE cloud_snapshots (
	id INTEGER NOT NULL, 
	server_id INTEGER NOT NULL, 
	external_id VARCHAR(128), 
	name VARCHAR(128), 
	size_gb FLOAT, 
	status VARCHAR(32), 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(server_id) REFERENCES cloud_servers (id)
);
CREATE TABLE pending_agents (
	id VARCHAR(36) NOT NULL, 
	enrollment_id VARCHAR(64) NOT NULL, 
	enrollment_secret_hash VARCHAR(256) NOT NULL, 
	pubkey VARCHAR(128) NOT NULL, 
	pubkey_fpr VARCHAR(32) NOT NULL, 
	pair_code VARCHAR(16) NOT NULL, 
	pair_code_expires_at DATETIME NOT NULL, 
	pair_code_frozen BOOLEAN NOT NULL, 
	passphrase_hash VARCHAR(256) NOT NULL, 
	lockout_until DATETIME, 
	failed_attempts INTEGER NOT NULL, 
	machine_id VARCHAR(128), 
	system_info JSON, 
	created_at DATETIME, 
	last_seen_at DATETIME, 
	expires_at DATETIME NOT NULL, 
	claimed_at DATETIME, 
	claimed_server_id VARCHAR(36), 
	claim_payload_encrypted TEXT, 
	PRIMARY KEY (id), 
	FOREIGN KEY(claimed_server_id) REFERENCES servers (id)
);
CREATE INDEX ix_pending_agents_pubkey_fpr ON pending_agents (pubkey_fpr);
CREATE INDEX ix_pending_agents_pair_code ON pending_agents (pair_code);
CREATE UNIQUE INDEX ix_pending_agents_enrollment_id ON pending_agents (enrollment_id);
CREATE INDEX ix_pending_agents_machine_id ON pending_agents (machine_id);
CREATE INDEX ix_pending_agents_created_at ON pending_agents (created_at);
CREATE TABLE secrets (
	id INTEGER NOT NULL, 
	vault_id INTEGER NOT NULL, 
	name VARCHAR(200) NOT NULL, 
	encrypted_value TEXT NOT NULL, 
	description TEXT, 
	expires_at DATETIME, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_secret_vault_name UNIQUE (vault_id, name), 
	FOREIGN KEY(vault_id) REFERENCES secret_vaults (id) ON DELETE CASCADE
);
CREATE INDEX ix_secrets_vault_id ON secrets (vault_id);
CREATE TABLE webhook_deliveries (
	id INTEGER NOT NULL, 
	endpoint_id INTEGER NOT NULL, 
	event_id VARCHAR(300) NOT NULL, 
	payload TEXT, 
	headers TEXT, 
	signature_valid BOOLEAN, 
	status VARCHAR(50), 
	response_status INTEGER, 
	response_body TEXT, 
	error_message TEXT, 
	received_at DATETIME, 
	completed_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(endpoint_id) REFERENCES webhook_endpoints (id) ON DELETE CASCADE
);
CREATE INDEX ix_webhook_deliveries_endpoint_id ON webhook_deliveries (endpoint_id);
CREATE UNIQUE INDEX ix_webhook_deliveries_event_id ON webhook_deliveries (event_id);
CREATE TABLE applications (
	id INTEGER NOT NULL, 
	name VARCHAR(100) NOT NULL, 
	app_type VARCHAR(50) NOT NULL, 
	status VARCHAR(20), 
	php_version VARCHAR(10), 
	python_version VARCHAR(10), 
	port INTEGER, 
	root_path VARCHAR(500), 
	docker_image VARCHAR(200), 
	container_id VARCHAR(100), 
	registry_id INTEGER, 
	buildpack_type VARCHAR(20), 
	buildpack_plan TEXT, 
	buildpack_overrides TEXT, 
	source VARCHAR(20) NOT NULL, 
	compose_file VARCHAR(200), 
	systemd_unit VARCHAR(100), 
	managed_by VARCHAR(20), 
	ingress_plane VARCHAR(20), 
	version INTEGER NOT NULL, 
	upload_path VARCHAR(500), 
	private_slug VARCHAR(50), 
	private_url_enabled BOOLEAN, 
	environment_type VARCHAR(20), 
	linked_app_id INTEGER, 
	shared_config TEXT, 
	created_at DATETIME, 
	updated_at DATETIME, 
	last_deployed_at DATETIME, 
	user_id INTEGER NOT NULL, 
	server_id VARCHAR(36), 
	workspace_id INTEGER, 
	project_id INTEGER, 
	environment_id INTEGER, 
	PRIMARY KEY (id), 
	FOREIGN KEY(registry_id) REFERENCES container_registries (id), 
	FOREIGN KEY(linked_app_id) REFERENCES applications (id), 
	FOREIGN KEY(user_id) REFERENCES users (id), 
	FOREIGN KEY(server_id) REFERENCES servers (id), 
	FOREIGN KEY(workspace_id) REFERENCES workspaces (id), 
	FOREIGN KEY(project_id) REFERENCES projects (id), 
	FOREIGN KEY(environment_id) REFERENCES environments (id)
);
CREATE INDEX ix_applications_environment_id ON applications (environment_id);
CREATE INDEX ix_applications_server_id ON applications (server_id);
CREATE UNIQUE INDEX ix_applications_private_slug ON applications (private_slug);
CREATE INDEX ix_applications_project_id ON applications (project_id);
CREATE INDEX ix_applications_registry_id ON applications (registry_id);
CREATE INDEX ix_applications_workspace_id ON applications (workspace_id);
CREATE TABLE exposed_services (
	id VARCHAR(36) NOT NULL, 
	tunnel_id VARCHAR(36) NOT NULL, 
	hostname VARCHAR(255) NOT NULL, 
	port INTEGER NOT NULL, 
	nginx_site_name VARCHAR(120), 
	require_auth BOOLEAN, 
	auth_username VARCHAR(100), 
	ssl_enabled BOOLEAN, 
	status VARCHAR(20), 
	last_error TEXT, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(tunnel_id) REFERENCES tunnels (id)
);
CREATE INDEX ix_exposed_services_tunnel_id ON exposed_services (tunnel_id);
CREATE INDEX ix_exposed_services_status ON exposed_services (status);
CREATE TABLE domains (
	id INTEGER NOT NULL, 
	name VARCHAR(255) NOT NULL, 
	is_primary BOOLEAN, 
	ssl_enabled BOOLEAN, 
	ssl_certificate_path VARCHAR(500), 
	ssl_key_path VARCHAR(500), 
	ssl_expires_at DATETIME, 
	ssl_auto_renew BOOLEAN, 
	created_at DATETIME, 
	updated_at DATETIME, 
	application_id INTEGER NOT NULL, 
	PRIMARY KEY (id), 
	FOREIGN KEY(application_id) REFERENCES applications (id)
);
CREATE UNIQUE INDEX ix_domains_name ON domains (name);
CREATE TABLE environment_variables (
	id INTEGER NOT NULL, 
	application_id INTEGER NOT NULL, 
	"key" VARCHAR(255) NOT NULL, 
	encrypted_value TEXT NOT NULL, 
	is_secret BOOLEAN, 
	description VARCHAR(500), 
	target_service VARCHAR(120), 
	created_at DATETIME, 
	updated_at DATETIME, 
	created_by INTEGER, 
	PRIMARY KEY (id), 
	CONSTRAINT unique_app_env_key UNIQUE (application_id, "key"), 
	FOREIGN KEY(application_id) REFERENCES applications (id), 
	FOREIGN KEY(created_by) REFERENCES users (id)
);
CREATE TABLE environment_variable_history (
	id INTEGER NOT NULL, 
	env_variable_id INTEGER NOT NULL, 
	application_id INTEGER NOT NULL, 
	"key" VARCHAR(255) NOT NULL, 
	action VARCHAR(20) NOT NULL, 
	old_value_hash VARCHAR(64), 
	new_value_hash VARCHAR(64), 
	changed_by INTEGER, 
	changed_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(application_id) REFERENCES applications (id), 
	FOREIGN KEY(changed_by) REFERENCES users (id)
);
CREATE TABLE deployments (
	id INTEGER NOT NULL, 
	app_id INTEGER NOT NULL, 
	version INTEGER NOT NULL, 
	version_tag VARCHAR(100), 
	status VARCHAR(20), 
	build_method VARCHAR(20), 
	image_tag VARCHAR(255), 
	commit_hash VARCHAR(40), 
	commit_message TEXT, 
	container_id VARCHAR(100), 
	deployed_by INTEGER, 
	deploy_trigger VARCHAR(20), 
	build_log_path VARCHAR(500), 
	created_at DATETIME, 
	build_started_at DATETIME, 
	build_completed_at DATETIME, 
	deploy_started_at DATETIME, 
	deploy_completed_at DATETIME, 
	error_message TEXT, 
	extra_data TEXT, 
	PRIMARY KEY (id), 
	FOREIGN KEY(app_id) REFERENCES applications (id), 
	FOREIGN KEY(deployed_by) REFERENCES users (id)
);
CREATE TABLE application_previews (
	id INTEGER NOT NULL, 
	application_id INTEGER NOT NULL, 
	pr_number INTEGER NOT NULL, 
	pr_title VARCHAR(500), 
	branch VARCHAR(255), 
	status VARCHAR(20) NOT NULL, 
	domain VARCHAR(255), 
	container_ids TEXT, 
	commit_sha VARCHAR(64), 
	expires_at DATETIME, 
	created_at DATETIME, 
	updated_at DATETIME, 
	deleted_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(application_id) REFERENCES applications (id)
);
CREATE INDEX ix_application_previews_application_id ON application_previews (application_id);
CREATE TABLE application_preview_settings (
	application_id INTEGER NOT NULL, 
	enabled BOOLEAN NOT NULL, 
	domain_template VARCHAR(255) NOT NULL, 
	target_server_id VARCHAR(36), 
	ttl_days INTEGER, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (application_id), 
	UNIQUE (application_id), 
	FOREIGN KEY(application_id) REFERENCES applications (id)
);
CREATE TABLE git_webhooks (
	id INTEGER NOT NULL, 
	name VARCHAR(100) NOT NULL, 
	source VARCHAR(50) NOT NULL, 
	source_repo_url VARCHAR(500) NOT NULL, 
	source_branch VARCHAR(100), 
	local_repo_name VARCHAR(200), 
	secret VARCHAR(100) NOT NULL, 
	webhook_token VARCHAR(50) NOT NULL, 
	sync_direction VARCHAR(20), 
	auto_sync BOOLEAN, 
	app_id INTEGER, 
	deploy_on_push BOOLEAN, 
	pre_deploy_script TEXT, 
	post_deploy_script TEXT, 
	zero_downtime BOOLEAN, 
	is_active BOOLEAN, 
	last_sync_at DATETIME, 
	last_sync_status VARCHAR(20), 
	last_sync_message TEXT, 
	sync_count INTEGER, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	UNIQUE (webhook_token), 
	FOREIGN KEY(app_id) REFERENCES applications (id)
);
CREATE TABLE wordpress_sites (
	id INTEGER NOT NULL, 
	application_id INTEGER NOT NULL, 
	wp_version VARCHAR(20), 
	multisite BOOLEAN, 
	admin_user VARCHAR(100), 
	admin_email VARCHAR(200), 
	db_name VARCHAR(100), 
	db_user VARCHAR(100), 
	db_host VARCHAR(200), 
	db_prefix VARCHAR(20), 
	git_repo_url VARCHAR(500), 
	git_branch VARCHAR(100), 
	git_paths TEXT, 
	auto_deploy BOOLEAN, 
	last_deploy_commit VARCHAR(40), 
	last_deploy_at DATETIME, 
	is_production BOOLEAN, 
	production_site_id INTEGER, 
	sync_config TEXT, 
	environment_type VARCHAR(20), 
	multidev_branch VARCHAR(200), 
	is_locked BOOLEAN, 
	locked_by VARCHAR(100), 
	locked_reason VARCHAR(200), 
	lock_expires_at DATETIME, 
	compose_project_name VARCHAR(100), 
	container_prefix VARCHAR(100), 
	resource_limits TEXT, 
	tags TEXT, 
	basic_auth_enabled BOOLEAN, 
	basic_auth_user VARCHAR(100), 
	basic_auth_password_hash VARCHAR(200), 
	health_status VARCHAR(20), 
	last_health_check DATETIME, 
	disk_usage_bytes BIGINT, 
	disk_usage_updated_at DATETIME, 
	last_vuln_scan_at DATETIME, 
	auto_update_schedule VARCHAR(100), 
	auto_update_exclude TEXT, 
	auto_sync_schedule VARCHAR(100), 
	auto_sync_enabled BOOLEAN, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	UNIQUE (application_id), 
	FOREIGN KEY(application_id) REFERENCES applications (id), 
	FOREIGN KEY(production_site_id) REFERENCES wordpress_sites (id)
);
CREATE TABLE app_volumes (
	id INTEGER NOT NULL, 
	application_id INTEGER NOT NULL, 
	name VARCHAR(120) NOT NULL, 
	docker_volume_name VARCHAR(200) NOT NULL, 
	mount_path VARCHAR(500) NOT NULL, 
	driver VARCHAR(40) NOT NULL, 
	read_only BOOLEAN NOT NULL, 
	size_bytes BIGINT, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_app_volume_mount UNIQUE (application_id, mount_path), 
	FOREIGN KEY(application_id) REFERENCES applications (id) ON DELETE CASCADE, 
	UNIQUE (docker_volume_name)
);
CREATE INDEX ix_app_volumes_application_id ON app_volumes (application_id);
CREATE TABLE managed_databases (
	id INTEGER NOT NULL, 
	engine VARCHAR(20) NOT NULL, 
	name VARCHAR(200) NOT NULL, 
	host_kind VARCHAR(20) NOT NULL, 
	container_ref VARCHAR(200), 
	host VARCHAR(255) NOT NULL, 
	port INTEGER, 
	owner_application_id INTEGER, 
	origin VARCHAR(20) NOT NULL, 
	admin_username VARCHAR(180), 
	admin_secret_encrypted TEXT, 
	workspace_id INTEGER, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_managed_database UNIQUE (engine, host, name), 
	FOREIGN KEY(owner_application_id) REFERENCES applications (id) ON DELETE SET NULL, 
	FOREIGN KEY(workspace_id) REFERENCES workspaces (id)
);
CREATE INDEX ix_managed_databases_created_at ON managed_databases (created_at);
CREATE INDEX ix_managed_databases_workspace_id ON managed_databases (workspace_id);
CREATE INDEX ix_managed_databases_engine ON managed_databases (engine);
CREATE INDEX ix_managed_databases_owner_application_id ON managed_databases (owner_application_id);
CREATE TABLE managed_dns_records (
	id INTEGER NOT NULL, 
	dns_provider_config_id INTEGER, 
	provider VARCHAR(64) NOT NULL, 
	provider_zone_id VARCHAR(128) NOT NULL, 
	provider_record_id VARCHAR(128), 
	record_type VARCHAR(10) NOT NULL, 
	name VARCHAR(256) NOT NULL, 
	content TEXT, 
	source VARCHAR(40), 
	app_id INTEGER, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(dns_provider_config_id) REFERENCES dns_provider_configs (id), 
	FOREIGN KEY(app_id) REFERENCES applications (id)
);
CREATE INDEX ix_managed_dns_records_provider_record_id ON managed_dns_records (provider_record_id);
CREATE INDEX ix_managed_dns_records_provider_zone_id ON managed_dns_records (provider_zone_id);
CREATE TABLE image_vulnerability_scans (
	id INTEGER NOT NULL, 
	application_id INTEGER NOT NULL, 
	image_ref VARCHAR(500) NOT NULL, 
	scanner VARCHAR(50), 
	scanner_version VARCHAR(50), 
	status VARCHAR(20), 
	severity_counts TEXT, 
	findings TEXT, 
	error_message TEXT, 
	started_at DATETIME, 
	completed_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(application_id) REFERENCES applications (id)
);
CREATE INDEX ix_image_vulnerability_scans_application_id ON image_vulnerability_scans (application_id);
CREATE TABLE sbom_artifacts (
	id INTEGER NOT NULL, 
	application_id INTEGER NOT NULL, 
	image_ref VARCHAR(500) NOT NULL, 
	generator VARCHAR(50), 
	generator_version VARCHAR(50), 
	format VARCHAR(20), 
	sbom_json TEXT, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(application_id) REFERENCES applications (id)
);
CREATE INDEX ix_sbom_artifacts_application_id ON sbom_artifacts (application_id);
CREATE TABLE waf_policies (
	id INTEGER NOT NULL, 
	application_id INTEGER NOT NULL, 
	mode VARCHAR(10) NOT NULL, 
	paranoia_level INTEGER NOT NULL, 
	anomaly_threshold INTEGER NOT NULL, 
	disabled_rule_ids TEXT, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(application_id) REFERENCES applications (id)
);
CREATE UNIQUE INDEX ix_waf_policies_application_id ON waf_policies (application_id);
CREATE TABLE image_update_checks (
	id INTEGER NOT NULL, 
	application_id INTEGER NOT NULL, 
	image_ref VARCHAR(500) NOT NULL, 
	current_digest VARCHAR(128), 
	latest_digest VARCHAR(128), 
	update_available BOOLEAN NOT NULL, 
	status VARCHAR(20), 
	error_message TEXT, 
	checked_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(application_id) REFERENCES applications (id)
);
CREATE INDEX ix_image_update_checks_application_id ON image_update_checks (application_id);
CREATE TABLE container_sleep_policies (
	id INTEGER NOT NULL, 
	application_id INTEGER NOT NULL, 
	enabled BOOLEAN NOT NULL, 
	idle_timeout_minutes INTEGER NOT NULL, 
	last_activity_at DATETIME, 
	asleep BOOLEAN NOT NULL, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(application_id) REFERENCES applications (id)
);
CREATE UNIQUE INDEX ix_container_sleep_policies_application_id ON container_sleep_policies (application_id);
CREATE TABLE container_scale_policies (
	id INTEGER NOT NULL, 
	application_id INTEGER NOT NULL, 
	enabled BOOLEAN NOT NULL, 
	service_name VARCHAR(100), 
	min_replicas INTEGER NOT NULL, 
	max_replicas INTEGER NOT NULL, 
	cpu_high_percent INTEGER NOT NULL, 
	cpu_low_percent INTEGER NOT NULL, 
	cooldown_seconds INTEGER NOT NULL, 
	current_replicas INTEGER NOT NULL, 
	last_scaled_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(application_id) REFERENCES applications (id)
);
CREATE UNIQUE INDEX ix_container_scale_policies_application_id ON container_scale_policies (application_id);
CREATE TABLE deployment_diffs (
	id INTEGER NOT NULL, 
	deployment_id INTEGER NOT NULL, 
	previous_deployment_id INTEGER, 
	files_added TEXT, 
	files_removed TEXT, 
	files_modified TEXT, 
	additions INTEGER, 
	deletions INTEGER, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(deployment_id) REFERENCES deployments (id), 
	FOREIGN KEY(previous_deployment_id) REFERENCES deployments (id)
);
CREATE TABLE deployment_snapshots (
	id INTEGER NOT NULL, 
	application_id INTEGER NOT NULL, 
	deployment_id INTEGER, 
	snapshot_hash VARCHAR(64) NOT NULL, 
	config_json TEXT NOT NULL, 
	summary VARCHAR(255), 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(application_id) REFERENCES applications (id), 
	FOREIGN KEY(deployment_id) REFERENCES deployments (id)
);
CREATE INDEX ix_deployment_snapshots_application_id ON deployment_snapshots (application_id);
CREATE INDEX ix_deployment_snapshots_deployment_id ON deployment_snapshots (deployment_id);
CREATE TABLE webhook_logs (
	id INTEGER NOT NULL, 
	webhook_id INTEGER, 
	source VARCHAR(50) NOT NULL, 
	event_type VARCHAR(50) NOT NULL, 
	delivery_id VARCHAR(100), 
	ref VARCHAR(200), 
	commit_sha VARCHAR(64), 
	commit_message TEXT, 
	pusher VARCHAR(100), 
	status VARCHAR(20), 
	status_message TEXT, 
	headers_json TEXT, 
	payload_preview TEXT, 
	received_at DATETIME, 
	processed_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(webhook_id) REFERENCES git_webhooks (id)
);
CREATE TABLE git_deployments (
	id INTEGER NOT NULL, 
	app_id INTEGER NOT NULL, 
	webhook_id INTEGER, 
	version INTEGER NOT NULL, 
	commit_sha VARCHAR(64), 
	commit_message TEXT, 
	branch VARCHAR(100), 
	triggered_by VARCHAR(100), 
	status VARCHAR(20), 
	started_at DATETIME, 
	completed_at DATETIME, 
	duration_seconds INTEGER, 
	pre_script_output TEXT, 
	deploy_output TEXT, 
	post_script_output TEXT, 
	error_message TEXT, 
	is_rollback BOOLEAN, 
	rollback_from_version INTEGER, 
	rolled_back_at DATETIME, 
	rolled_back_to_id INTEGER, 
	snapshot_data TEXT, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(app_id) REFERENCES applications (id), 
	FOREIGN KEY(webhook_id) REFERENCES git_webhooks (id)
);
CREATE TABLE database_snapshots (
	id INTEGER NOT NULL, 
	site_id INTEGER NOT NULL, 
	name VARCHAR(200) NOT NULL, 
	description TEXT, 
	tag VARCHAR(100), 
	file_path VARCHAR(500) NOT NULL, 
	size_bytes BIGINT, 
	compressed BOOLEAN, 
	commit_sha VARCHAR(40), 
	commit_message TEXT, 
	tables_included TEXT, 
	row_count INTEGER, 
	status VARCHAR(20), 
	error_message TEXT, 
	created_at DATETIME, 
	expires_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(site_id) REFERENCES wordpress_sites (id)
);
CREATE TABLE sync_jobs (
	id INTEGER NOT NULL, 
	source_site_id INTEGER NOT NULL, 
	target_site_id INTEGER NOT NULL, 
	name VARCHAR(200), 
	schedule VARCHAR(100), 
	enabled BOOLEAN, 
	config TEXT, 
	last_run DATETIME, 
	last_run_status VARCHAR(20), 
	last_run_duration INTEGER, 
	last_run_error TEXT, 
	next_run DATETIME, 
	run_count INTEGER, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(source_site_id) REFERENCES wordpress_sites (id), 
	FOREIGN KEY(target_site_id) REFERENCES wordpress_sites (id)
);
CREATE TABLE wordpress_vulnerabilities (
	id INTEGER NOT NULL, 
	site_id INTEGER NOT NULL, 
	source VARCHAR(20) NOT NULL, 
	slug VARCHAR(200), 
	name VARCHAR(255), 
	installed_version VARCHAR(50), 
	advisory_id VARCHAR(100), 
	title TEXT, 
	severity VARCHAR(20), 
	cvss_score VARCHAR(10), 
	fixed_in VARCHAR(50), 
	reference_url VARCHAR(500), 
	detected_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(site_id) REFERENCES wordpress_sites (id)
);
CREATE INDEX ix_wordpress_vulnerabilities_site_id ON wordpress_vulnerabilities (site_id);
CREATE TABLE wordpress_update_runs (
	id INTEGER NOT NULL, 
	site_id INTEGER NOT NULL, 
	status VARCHAR(20), 
	"trigger" VARCHAR(20), 
	details TEXT, 
	error TEXT, 
	started_at DATETIME, 
	finished_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(site_id) REFERENCES wordpress_sites (id)
);
CREATE INDEX ix_wordpress_update_runs_site_id ON wordpress_update_runs (site_id);
CREATE TABLE wordpress_reports (
	id INTEGER NOT NULL, 
	site_id INTEGER NOT NULL, 
	period_label VARCHAR(7) NOT NULL, 
	period_start DATETIME NOT NULL, 
	period_end DATETIME NOT NULL, 
	data TEXT, 
	generated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(site_id) REFERENCES wordpress_sites (id)
);
CREATE INDEX ix_wordpress_reports_site_id ON wordpress_reports (site_id);
CREATE TABLE wordpress_site_plugins (
	id INTEGER NOT NULL, 
	wordpress_site_id INTEGER NOT NULL, 
	custom_plugin_id INTEGER NOT NULL, 
	installed_version VARCHAR(50), 
	status VARCHAR(20), 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	CONSTRAINT uq_site_custom_plugin UNIQUE (wordpress_site_id, custom_plugin_id), 
	FOREIGN KEY(wordpress_site_id) REFERENCES wordpress_sites (id), 
	FOREIGN KEY(custom_plugin_id) REFERENCES wordpress_custom_plugins (id)
);
CREATE INDEX ix_wordpress_site_plugins_custom_plugin_id ON wordpress_site_plugins (custom_plugin_id);
CREATE INDEX ix_wordpress_site_plugins_wordpress_site_id ON wordpress_site_plugins (wordpress_site_id);
CREATE TABLE environment_activities (
	id INTEGER NOT NULL, 
	site_id INTEGER NOT NULL, 
	user_id INTEGER, 
	action VARCHAR(50) NOT NULL, 
	description TEXT, 
	metadata TEXT, 
	status VARCHAR(20), 
	error_message TEXT, 
	duration_seconds FLOAT, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(site_id) REFERENCES wordpress_sites (id), 
	FOREIGN KEY(user_id) REFERENCES users (id)
);
CREATE TABLE status_components (
	id INTEGER NOT NULL, 
	page_id INTEGER NOT NULL, 
	name VARCHAR(128) NOT NULL, 
	description TEXT, 
	"group" VARCHAR(64), 
	sort_order INTEGER, 
	check_type VARCHAR(16), 
	check_target VARCHAR(512), 
	check_interval INTEGER, 
	check_timeout INTEGER, 
	wordpress_site_id INTEGER, 
	status VARCHAR(32), 
	last_check_at DATETIME, 
	last_response_time INTEGER, 
	uptime_24h FLOAT, 
	uptime_7d FLOAT, 
	uptime_30d FLOAT, 
	uptime_90d FLOAT, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(page_id) REFERENCES status_pages (id), 
	FOREIGN KEY(wordpress_site_id) REFERENCES wordpress_sites (id)
);
CREATE TABLE deployment_jobs (
	id VARCHAR(36) NOT NULL, 
	kind VARCHAR(50) NOT NULL, 
	status VARCHAR(20) NOT NULL, 
	target_server_id VARCHAR(36), 
	app_id INTEGER, 
	requested_by INTEGER, 
	"trigger" VARCHAR(30), 
	deployment_id INTEGER, 
	git_deployment_id INTEGER, 
	webhook_id INTEGER, 
	commit_hash VARCHAR(40), 
	image_tag VARCHAR(255), 
	container_id VARCHAR(100), 
	total_steps INTEGER, 
	current_step INTEGER, 
	current_step_name VARCHAR(200), 
	"plan" TEXT, 
	result TEXT, 
	error_message TEXT, 
	correlation_id VARCHAR(64), 
	created_at DATETIME, 
	started_at DATETIME, 
	completed_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(target_server_id) REFERENCES servers (id), 
	FOREIGN KEY(app_id) REFERENCES applications (id), 
	FOREIGN KEY(requested_by) REFERENCES users (id), 
	FOREIGN KEY(deployment_id) REFERENCES deployments (id), 
	FOREIGN KEY(git_deployment_id) REFERENCES git_deployments (id), 
	FOREIGN KEY(webhook_id) REFERENCES git_webhooks (id)
);
CREATE INDEX ix_deployment_jobs_app_id ON deployment_jobs (app_id);
CREATE INDEX ix_deployment_jobs_target_server_id ON deployment_jobs (target_server_id);
CREATE INDEX ix_deployment_jobs_deployment_id ON deployment_jobs (deployment_id);
CREATE INDEX ix_deployment_jobs_created_at ON deployment_jobs (created_at);
CREATE INDEX ix_deployment_jobs_kind ON deployment_jobs (kind);
CREATE INDEX ix_deployment_jobs_webhook_id ON deployment_jobs (webhook_id);
CREATE INDEX ix_deployment_jobs_status ON deployment_jobs (status);
CREATE INDEX ix_deployment_jobs_git_deployment_id ON deployment_jobs (git_deployment_id);
CREATE INDEX ix_deployment_jobs_correlation_id ON deployment_jobs (correlation_id);
CREATE TABLE promotion_jobs (
	id INTEGER NOT NULL, 
	source_site_id INTEGER NOT NULL, 
	target_site_id INTEGER NOT NULL, 
	user_id INTEGER, 
	promotion_type VARCHAR(20) NOT NULL, 
	config TEXT, 
	status VARCHAR(20), 
	pre_promotion_snapshot_id INTEGER, 
	error_message TEXT, 
	started_at DATETIME, 
	completed_at DATETIME, 
	duration_seconds FLOAT, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(source_site_id) REFERENCES wordpress_sites (id), 
	FOREIGN KEY(target_site_id) REFERENCES wordpress_sites (id), 
	FOREIGN KEY(user_id) REFERENCES users (id), 
	FOREIGN KEY(pre_promotion_snapshot_id) REFERENCES database_snapshots (id)
);
CREATE TABLE health_checks (
	id INTEGER NOT NULL, 
	component_id INTEGER NOT NULL, 
	status VARCHAR(16), 
	response_time INTEGER, 
	status_code INTEGER, 
	error TEXT, 
	checked_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(component_id) REFERENCES status_components (id)
);
CREATE TABLE status_incidents (
	id INTEGER NOT NULL, 
	page_id INTEGER NOT NULL, 
	component_id INTEGER, 
	title VARCHAR(256) NOT NULL, 
	status VARCHAR(32), 
	impact VARCHAR(32), 
	body TEXT, 
	is_maintenance BOOLEAN, 
	scheduled_start DATETIME, 
	scheduled_end DATETIME, 
	resolved_at DATETIME, 
	created_at DATETIME, 
	updated_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(page_id) REFERENCES status_pages (id), 
	FOREIGN KEY(component_id) REFERENCES status_components (id)
);
CREATE TABLE deployment_job_logs (
	id BIGINT NOT NULL, 
	job_id VARCHAR(36) NOT NULL, 
	step_index INTEGER, 
	level VARCHAR(10), 
	message TEXT NOT NULL, 
	data TEXT, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(job_id) REFERENCES deployment_jobs (id)
);
CREATE INDEX ix_deployment_job_logs_created_at ON deployment_job_logs (created_at);
CREATE INDEX ix_deployment_job_logs_job_id ON deployment_job_logs (job_id);
CREATE TABLE status_incident_updates (
	id INTEGER NOT NULL, 
	incident_id INTEGER NOT NULL, 
	status VARCHAR(32), 
	body TEXT NOT NULL, 
	created_at DATETIME, 
	PRIMARY KEY (id), 
	FOREIGN KEY(incident_id) REFERENCES status_incidents (id)
);
