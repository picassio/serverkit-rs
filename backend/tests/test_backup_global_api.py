"""§8 Phase B3 — global, queryable backup policy/run listing endpoints.

GET /api/v1/backups/policies and /api/v1/backups/runs give the unified Backups
page one cross-target view, replacing the legacy per-type filesystem scans.
"""
from datetime import datetime


def _seed(app):
    from app import db
    from app.models.backup_policy import BackupPolicy
    from app.models.backup_run import BackupRun
    with app.app_context():
        app_policy = BackupPolicy(target_type='application', target_id=1, enabled=True)
        db_policy = BackupPolicy(target_type='database', target_id=2, target_subtype='mysql',
                                 enabled=False)
        db.session.add_all([app_policy, db_policy])
        db.session.commit()
        db.session.add(BackupRun(policy_id=app_policy.id, kind='full', status='success',
                                 started_at=datetime.utcnow(), size_local=100))
        db.session.add(BackupRun(policy_id=db_policy.id, kind='full', status='failed',
                                 started_at=datetime.utcnow(), size_local=0))
        db.session.commit()
        return app_policy.id, db_policy.id


def test_list_policies_returns_all(app, client, auth_headers):
    _seed(app)
    res = client.get('/api/v1/backups/policies', headers=auth_headers)
    assert res.status_code == 200
    policies = res.get_json()['policies']
    types = {p['target_type'] for p in policies}
    assert {'application', 'database'} <= types


def test_list_policies_filters_by_target_type(app, client, auth_headers):
    _seed(app)
    res = client.get('/api/v1/backups/policies?target_type=database', headers=auth_headers)
    assert res.status_code == 200
    policies = res.get_json()['policies']
    assert policies and all(p['target_type'] == 'database' for p in policies)


def test_list_policies_filters_by_enabled(app, client, auth_headers):
    _seed(app)
    res = client.get('/api/v1/backups/policies?enabled=true', headers=auth_headers)
    assert res.status_code == 200
    policies = res.get_json()['policies']
    assert policies and all(p['enabled'] for p in policies)


def test_list_policies_rejects_bad_target_type(app, client, auth_headers):
    _seed(app)
    res = client.get('/api/v1/backups/policies?target_type=bogus', headers=auth_headers)
    assert res.status_code == 400


def test_list_runs_augments_target_info(app, client, auth_headers):
    _seed(app)
    res = client.get('/api/v1/backups/runs', headers=auth_headers)
    assert res.status_code == 200
    runs = res.get_json()['runs']
    assert len(runs) == 2
    assert all('target_type' in r and 'target_id' in r for r in runs)


def test_list_runs_filters_by_status_and_type(app, client, auth_headers):
    _seed(app)
    res = client.get('/api/v1/backups/runs?target_type=database&status=failed', headers=auth_headers)
    assert res.status_code == 200
    runs = res.get_json()['runs']
    assert runs and all(r['target_type'] == 'database' and r['status'] == 'failed' for r in runs)
