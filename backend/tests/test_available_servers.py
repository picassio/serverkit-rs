"""/servers/available feeds the target pickers (File Manager, Cron, …) — it
must carry os_type so target-aware UIs can offer paths that actually exist on
the selected box (a Windows agent has no /var/www)."""
from app import db
from app.models.server import Server
from app.services.remote_docker_service import RemoteDockerService


def _make_server(**overrides):
    data = dict(
        name='Test Agent',
        hostname='agent.example.com',
        ip_address='203.0.113.20',
        status='online',
    )
    data.update(overrides)
    server = Server(**data)
    db.session.add(server)
    db.session.commit()
    return server


def test_available_servers_include_os_type(app):
    with app.app_context():
        _make_server(name='Win Agent', os_type='windows')

        servers = RemoteDockerService.get_available_servers()
        remote = [s for s in servers if not s['is_local']]
        assert remote, 'online server missing from the available list'
        assert remote[0]['os_type'] == 'windows'


def test_available_servers_os_type_null_for_legacy_agents(app):
    """Agents enrolled before sysinfo reporting have no os_type — the field
    must still be present (null), not absent, so consumers can key off it."""
    with app.app_context():
        _make_server(name='Old Agent', os_type=None)

        servers = RemoteDockerService.get_available_servers()
        remote = [s for s in servers if not s['is_local']]
        assert remote
        assert 'os_type' in remote[0]
        assert remote[0]['os_type'] is None


def test_agent_footprint_round_trip(app):
    """The agent's self-reported install_dir/config_dir (system_info) land on
    the Server row via update_system_info and surface in /servers/available —
    the chain the File Manager's per-target quick links ride on."""
    from app.services.agent_registry import agent_registry

    with app.app_context():
        server = _make_server(name='Reporting Agent', os_type='linux')

        agent_registry.update_system_info(server.id, {
            'install_dir': '/usr/local/bin',
            'config_dir': '/etc/serverkit-agent',
        })

        db.session.refresh(server)
        assert server.agent_install_dir == '/usr/local/bin'
        assert server.agent_config_dir == '/etc/serverkit-agent'

        remote = [s for s in RemoteDockerService.get_available_servers()
                  if not s['is_local']]
        assert remote[0]['agent_install_dir'] == '/usr/local/bin'
        assert remote[0]['agent_config_dir'] == '/etc/serverkit-agent'

        # A later payload without the keys (transient probe gap) must not
        # wipe the stored values — same coalesce contract as the other
        # system_info columns.
        agent_registry.update_system_info(server.id, {'hostname': 'still-here'})
        db.session.refresh(server)
        assert server.agent_config_dir == '/etc/serverkit-agent'
