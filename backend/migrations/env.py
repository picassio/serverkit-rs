import logging
from logging.config import fileConfig

from flask import current_app
from alembic import context

# Alembic Config object
config = context.config

# Set up loggers
if config.config_file_name is not None:
    fileConfig(config.config_file_name)
logger = logging.getLogger('alembic.env')


def get_engine():
    try:
        # Flask-Migrate provides the engine via current_app
        return current_app.extensions['migrate'].db.get_engine()
    except (TypeError, AttributeError):
        # Fallback for CLI usage outside Flask context
        return current_app.extensions['migrate'].db.engine


def get_engine_url():
    try:
        return get_engine().url.render_as_string(hide_password=False).replace('%', '%%')
    except AttributeError:
        return str(get_engine().url).replace('%', '%%')


# Import all models so Alembic can detect them
def import_models():
    # noinspection PyUnresolvedReferences
    from app.models import (  # noqa: F401
        User, Application, Domain, EnvironmentVariable, EnvironmentVariableHistory,
        NotificationPreferences, Deployment, DeploymentDiff, SystemSettings, AuditLog,
        MetricsHistory, Workflow, GitWebhook, WebhookLog, GitDeployment,
        Server, ServerGroup, ServerMetrics, ServerCommand, AgentSession, SecurityAlert,
        WordPressSite, DatabaseSnapshot, SyncJob,
        EnvironmentActivity, PromotionJob, SanitizationProfile, EmailAccount,
        OAuthIdentity
    )


config.set_main_option('sqlalchemy.url', get_engine_url())
target_db = current_app.extensions['migrate'].db


def get_metadata():
    if hasattr(target_db, 'metadatas'):
        return target_db.metadatas[None]
    return target_db.metadata


def run_migrations_offline():
    """Run migrations in 'offline' mode."""
    url = config.get_main_option("sqlalchemy.url")
    context.configure(
        url=url,
        target_metadata=get_metadata(),
        literal_binds=True,
    )

    import_models()

    with context.begin_transaction():
        context.run_migrations()


def run_migrations_online():
    """Run migrations in 'online' mode."""

    def process_revision_directives(context, revision, directives):
        if getattr(config.cmd_opts, 'autogenerate', False):
            script = directives[0]
            if script.upgrade_ops.is_empty():
                directives[:] = []
                logger.info('No changes in schema detected.')

    connectable = get_engine()

    import_models()

    with connectable.connect() as connection:
        context.configure(
            connection=connection,
            target_metadata=get_metadata(),
            process_revision_directives=process_revision_directives,
            render_as_batch=True,  # Required for SQLite ALTER TABLE support
        )

        with context.begin_transaction():
            context.run_migrations()


if context.is_offline_mode():
    run_migrations_offline()
else:
    run_migrations_online()
