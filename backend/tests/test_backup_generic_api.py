"""§8 Phase B5 — generic backup-policy CRUD by (target_type, target_id).

One admin surface drives policies for every target type, including the
database/files/server targets that have no per-resource page. The existing
/apps/:id/backup-policy and /wordpress/sites/:id/backup-policy mounts are
unchanged.
"""


def test_put_then_get_database_policy(app, client, auth_headers):
    body = {
        'enabled': True,
        'schedule_cron': '0 3 * * *',
        'target_subtype': 'mysql',
        'target_meta': {'db_name': 'shop', 'host': 'localhost'},
    }
    put = client.put('/api/v1/backups/policies/database/5', json=body, headers=auth_headers)
    assert put.status_code == 200, put.get_json()
    view = put.get_json()
    assert view['policy']['target_type'] == 'database'
    assert view['policy']['target_subtype'] == 'mysql'
    assert view['policy']['target_meta']['db_name'] == 'shop'
    assert view['policy']['enabled'] is True

    get = client.get('/api/v1/backups/policies/database/5', headers=auth_headers)
    assert get.status_code == 200
    assert get.get_json()['policy']['schedule_cron'] == '0 3 * * *'


def test_get_creates_default_files_policy(app, client, auth_headers):
    get = client.get('/api/v1/backups/policies/files/9', headers=auth_headers)
    assert get.status_code == 200
    assert get.get_json()['policy']['target_type'] == 'files'
    assert get.get_json()['policy']['enabled'] is False


def test_invalid_target_type_rejected(app, client, auth_headers):
    res = client.get('/api/v1/backups/policies/bogus/1', headers=auth_headers)
    assert res.status_code == 400


def test_list_runs_empty_for_new_policy(app, client, auth_headers):
    res = client.get('/api/v1/backups/policies/files/77/runs', headers=auth_headers)
    assert res.status_code == 200
    assert res.get_json()['runs'] == []


def test_generic_surface_requires_admin(app, client):
    # No auth → rejected (matches the rest of /backups).
    res = client.get('/api/v1/backups/policies/database/1')
    assert res.status_code in (401, 422)
