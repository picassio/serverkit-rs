import json
import logging
from datetime import datetime
from app import db
from app.models.agent_plugin import AgentPlugin, AgentPluginInstall
from app.models.server import Server

logger = logging.getLogger(__name__)


class AgentPluginService:
    """Service for managing agent plugins."""

    # Plugin specification interface
    PLUGIN_SPEC = {
        'required_fields': ['name', 'display_name', 'version'],
        'capability_types': ['metrics', 'health_checks', 'commands', 'scheduled_tasks', 'event_hooks'],
        'permission_types': ['filesystem', 'network', 'docker', 'process', 'system'],
    }

    @staticmethod
    def list_plugins(status=None):
        query = AgentPlugin.query
        if status:
            query = query.filter_by(status=status)
        return query.order_by(AgentPlugin.display_name).all()

    @staticmethod
    def get_plugin(plugin_id):
        return AgentPlugin.query.get(plugin_id)

    @staticmethod
    def get_plugin_by_name(name):
        return AgentPlugin.query.filter_by(name=name).first()

    @staticmethod
    def create_plugin(data):
        """Register a new plugin from manifest data."""
        if AgentPlugin.query.filter_by(name=data['name']).first():
            raise ValueError(f"Plugin '{data['name']}' already exists")

        plugin = AgentPlugin(
            name=data['name'],
            display_name=data.get('display_name', data['name']),
            version=data['version'],
            description=data.get('description', ''),
            author=data.get('author', ''),
            homepage=data.get('homepage', ''),
            max_memory_mb=data.get('max_memory_mb', 128),
            max_cpu_percent=data.get('max_cpu_percent', 10),
        )
        plugin.manifest = data.get('manifest', data)
        plugin.capabilities = data.get('capabilities', [])
        plugin.dependencies = data.get('dependencies', [])
        plugin.permissions = data.get('permissions', [])

        db.session.add(plugin)
        db.session.commit()
        return plugin

    @staticmethod
    def update_plugin(plugin_id, data):
        plugin = AgentPlugin.query.get(plugin_id)
        if not plugin:
            return None

        for field in ['display_name', 'version', 'description', 'author', 'homepage',
                      'max_memory_mb', 'max_cpu_percent', 'status']:
            if field in data:
                setattr(plugin, field, data[field])

        if 'capabilities' in data:
            plugin.capabilities = data['capabilities']
        if 'dependencies' in data:
            plugin.dependencies = data['dependencies']
        if 'permissions' in data:
            plugin.permissions = data['permissions']
        if 'manifest' in data:
            plugin.manifest = data['manifest']

        db.session.commit()
        return plugin

    @staticmethod
    def delete_plugin(plugin_id):
        plugin = AgentPlugin.query.get(plugin_id)
        if not plugin:
            return False

        # Check for active installations
        active = plugin.installations.filter(
            AgentPluginInstall.status.in_([
                AgentPluginInstall.STATUS_ENABLED,
                AgentPluginInstall.STATUS_INSTALLING
            ])
        ).count()
        if active > 0:
            raise ValueError(f'Cannot delete plugin with {active} active installations')

        # Remove all installation records
        AgentPluginInstall.query.filter_by(plugin_id=plugin_id).delete()
        db.session.delete(plugin)
        db.session.commit()
        return True

    # --- Installation Management ---

    @staticmethod
    def install_plugin(plugin_id, server_id, config=None):
        """Install a plugin on a server."""
        plugin = AgentPlugin.query.get(plugin_id)
        if not plugin:
            raise ValueError('Plugin not found')

        server = Server.query.get(server_id)
        if not server:
            raise ValueError('Server not found')

        # Check if already installed
        existing = AgentPluginInstall.query.filter_by(
            plugin_id=plugin_id, server_id=server_id
        ).first()
        if existing and existing.status in ['enabled', 'installing']:
            raise ValueError('Plugin already installed on this server')

        # Check dependencies
        for dep_name in plugin.dependencies:
            dep_plugin = AgentPlugin.query.filter_by(name=dep_name).first()
            if not dep_plugin:
                raise ValueError(f"Required dependency '{dep_name}' not available")
            dep_install = AgentPluginInstall.query.filter_by(
                plugin_id=dep_plugin.id, server_id=server_id, status='enabled'
            ).first()
            if not dep_install:
                raise ValueError(f"Dependency '{dep_name}' not installed on server")

        if existing:
            existing.status = AgentPluginInstall.STATUS_INSTALLING
            existing.installed_version = plugin.version
            existing.error_message = None
            if config:
                existing.config = config
            install = existing
        else:
            install = AgentPluginInstall(
                plugin_id=plugin_id,
                server_id=server_id,
                installed_version=plugin.version,
                status=AgentPluginInstall.STATUS_INSTALLING,
            )
            if config:
                install.config = config
            db.session.add(install)

        # Agent-side plugin execution is NOT implemented: the Go agent has no
        # plugin_install handler and there is no panel gateway to route one to.
        # The previous code called a non-existent get_agent_gateway() and
        # swallowed the resulting ImportError, leaving the row stuck in
        # 'installing' forever. Fail honestly so the UI reflects reality.
        install.status = AgentPluginInstall.STATUS_ERROR
        install.error_message = 'Agent-side plugin support is not implemented yet'
        db.session.commit()

        return install

    @staticmethod
    def uninstall_plugin(install_id):
        install = AgentPluginInstall.query.get(install_id)
        if not install:
            return False

        # No agent-side plugin runtime exists to notify; uninstall is local-only
        # bookkeeping. (Previously dispatched to a non-existent get_agent_gateway.)
        install.status = AgentPluginInstall.STATUS_UNINSTALLING
        db.session.commit()

        return True

    @staticmethod
    def enable_plugin(install_id):
        install = AgentPluginInstall.query.get(install_id)
        if not install:
            return None
        install.status = AgentPluginInstall.STATUS_ENABLED
        db.session.commit()
        return install

    @staticmethod
    def disable_plugin(install_id):
        install = AgentPluginInstall.query.get(install_id)
        if not install:
            return None
        # Local-only state change; no agent-side plugin runtime to notify.
        install.status = AgentPluginInstall.STATUS_DISABLED
        db.session.commit()

        return install

    @staticmethod
    def update_install_status(install_id, status, error=None, health=None, metrics=None):
        install = AgentPluginInstall.query.get(install_id)
        if not install:
            return None
        install.status = status
        if error is not None:
            install.error_message = error
        if health is not None:
            install.health_status = health
            install.last_health_check = datetime.utcnow()
        if metrics is not None:
            install.metrics_json = json.dumps(metrics)
        db.session.commit()
        return install

    @staticmethod
    def update_install_config(install_id, config):
        install = AgentPluginInstall.query.get(install_id)
        if not install:
            return None
        # Local-only config persistence; no agent-side plugin runtime to notify.
        install.config = config
        db.session.commit()

        return install

    @staticmethod
    def get_server_plugins(server_id):
        return AgentPluginInstall.query.filter_by(server_id=server_id).all()

    @staticmethod
    def get_plugin_installations(plugin_id):
        return AgentPluginInstall.query.filter_by(plugin_id=plugin_id).all()

    @staticmethod
    def get_install(install_id):
        return AgentPluginInstall.query.get(install_id)

    @staticmethod
    def bulk_install(plugin_id, server_ids, config=None):
        """Install plugin on multiple servers."""
        results = []
        for sid in server_ids:
            try:
                install = AgentPluginService.install_plugin(plugin_id, sid, config)
                results.append({'server_id': sid, 'status': 'installing', 'install_id': install.id})
            except ValueError as e:
                results.append({'server_id': sid, 'status': 'error', 'error': str(e)})
        return results

    @staticmethod
    def validate_manifest(manifest):
        """Validate a plugin manifest against the spec."""
        errors = []
        for field in AgentPluginService.PLUGIN_SPEC['required_fields']:
            if field not in manifest:
                errors.append(f"Missing required field: {field}")

        for cap in manifest.get('capabilities', []):
            if cap not in AgentPluginService.PLUGIN_SPEC['capability_types']:
                errors.append(f"Unknown capability: {cap}")

        for perm in manifest.get('permissions', []):
            if perm not in AgentPluginService.PLUGIN_SPEC['permission_types']:
                errors.append(f"Unknown permission: {perm}")

        return errors
