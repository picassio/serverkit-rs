"""Phase 0 proving tests.

The Database Explorer must tell a genuinely empty database apart from a
container/auth failure. Both used to collapse to an empty table list and render
"Empty", silently hiding a broken connection.
"""


def test_docker_get_tables_reports_connection_failure(monkeypatch):
    from app.services.database_service import DatabaseService
    monkeypatch.setattr(DatabaseService, 'docker_mysql_execute',
                        staticmethod(lambda *a, **k: {'success': False, 'error': 'No such container: ghost'}))
    res = DatabaseService.docker_mysql_get_tables('ghost', 'wordpress')
    assert res['connected'] is False
    assert res['tables'] == []
    assert 'ghost' in res['error']


def test_docker_get_tables_empty_db_is_connected(monkeypatch):
    from app.services.database_service import DatabaseService
    monkeypatch.setattr(DatabaseService, 'docker_mysql_execute',
                        staticmethod(lambda *a, **k: {'success': True, 'output': '', 'error': None}))
    res = DatabaseService.docker_mysql_get_tables('c', 'wordpress')
    # Connected but no tables — the case the UI now renders as "No tables yet".
    assert res['connected'] is True
    assert res['tables'] == []


def test_docker_get_tables_lists_tables_with_counts(monkeypatch):
    from app.services.database_service import DatabaseService

    def fake_exec(container, query, database=None, user='root', password=None):
        if 'SHOW TABLES' in query:
            return {'success': True, 'output': 'Tables_in_wordpress\nwp_posts\nwp_options\n', 'error': None}
        return {'success': True, 'output': 'COUNT(*)\n7\n', 'error': None}

    monkeypatch.setattr(DatabaseService, 'docker_mysql_execute', staticmethod(fake_exec))
    res = DatabaseService.docker_mysql_get_tables('c', 'wordpress')
    assert res['connected'] is True
    assert [t['name'] for t in res['tables']] == ['wp_posts', 'wp_options']
    assert res['tables'][0]['rows'] == 7


def test_docker_tables_api_surfaces_connection_state(app, client, auth_headers, monkeypatch):
    from app.services.database_service import DatabaseService
    monkeypatch.setattr(DatabaseService, 'docker_mysql_get_tables',
                        staticmethod(lambda *a, **k: {'connected': False, 'tables': [], 'error': 'boom'}))
    r = client.get('/api/v1/databases/docker/ghost/wordpress/tables', headers=auth_headers)
    assert r.status_code == 200
    body = r.get_json()
    assert body['connected'] is False
    assert body['error'] == 'boom'
    assert body['tables'] == []
