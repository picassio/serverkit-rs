from app.models.server import Server


def test_read_profiles_cover_agent_read_actions():
    server = Server(
        name='test-server',
        permissions=[
            'docker:container:read',
            'docker:image:read',
            'docker:volume:read',
            'docker:network:read',
            'docker:compose:read',
            'system:metrics:read',
        ],
    )

    assert server.has_permission('docker:container:list')
    assert server.has_permission('docker:container:inspect')
    assert server.has_permission('docker:container:logs')
    assert server.has_permission('docker:image:list')
    assert server.has_permission('docker:volume:list')
    assert server.has_permission('docker:network:list')
    assert server.has_permission('docker:compose:ps')
    assert server.has_permission('system:metrics')
    assert server.has_permission('system:info')
    assert not server.has_permission('docker:container:start')
    assert not server.has_permission('docker:image:pull')


def test_legacy_ui_permissions_still_work_for_existing_servers():
    server = Server(
        name='test-server',
        permissions=['docker:read', 'docker:write', 'system:read'],
    )

    assert server.has_permission('docker:container:list')
    assert server.has_permission('docker:container:start')
    assert server.has_permission('docker:image:pull')
    assert server.has_permission('system:metrics')
    assert server.has_permission('system:info')


def test_wildcard_permissions_still_match_agent_actions():
    server = Server(name='test-server', permissions=['docker:container:*'])

    assert server.has_permission('docker:container:list')
    assert server.has_permission('docker:container:restart')
    assert not server.has_permission('docker:image:list')
