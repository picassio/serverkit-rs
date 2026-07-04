"""§8 Phase B2 — database + files executors in the unified backup service.

Verifies that BackupPolicyService routes database/files targets to the right
BackupService primitives and builds correct run metadata, without needing a
live database or real archives (BackupService is mocked).
"""
import pytest

from app.services.backup_policy_service import BackupPolicyService, BackupPolicyError


# --- target resolution ----------------------------------------------------- #

def test_resolve_database_target(app):
    with app.app_context():
        policy = BackupPolicyService.get_or_create_policy(
            'database', 7, target_subtype='postgresql',
            target_meta={'db_name': 'shop', 'user': 'app', 'host': 'db1'},
        )
        target = BackupPolicyService._resolve_target(policy)
        assert target['target_type'] == 'database'
        assert target['name'] == 'shop'
        assert target['db_config']['db_type'] == 'postgresql'  # from subtype
        assert target['db_config']['db_name'] == 'shop'
        assert target['db_config']['host'] == 'db1'


def test_resolve_database_requires_db_name(app):
    with app.app_context():
        policy = BackupPolicyService.get_or_create_policy('database', 8, target_meta={})
        with pytest.raises(BackupPolicyError):
            BackupPolicyService._resolve_target(policy)


def test_resolve_files_target(app):
    with app.app_context():
        policy = BackupPolicyService.get_or_create_policy(
            'files', 3, target_meta={'paths': ['/etc/nginx', '/srv/data'], 'label': 'configs'},
        )
        target = BackupPolicyService._resolve_target(policy)
        assert target['target_type'] == 'files'
        assert target['name'] == 'configs'
        assert target['file_paths'] == ['/etc/nginx', '/srv/data']


def test_resolve_files_requires_paths(app):
    with app.app_context():
        policy = BackupPolicyService.get_or_create_policy('files', 4, target_meta={'paths': []})
        with pytest.raises(BackupPolicyError):
            BackupPolicyService._resolve_target(policy)


def test_resolve_server_not_implemented(app):
    with app.app_context():
        policy = BackupPolicyService.get_or_create_policy('server', 1)
        with pytest.raises(BackupPolicyError):
            BackupPolicyService._resolve_target(policy)


# --- execution routing (BackupService mocked) ------------------------------ #

def test_execute_database_backup_routes_and_builds_meta(app, monkeypatch):
    from app.services.backup_service import BackupService
    calls = {}

    def fake_backup_database(db_type, db_name, user=None, password=None, host='localhost'):
        calls['db'] = (db_type, db_name, user, host)
        return {'success': True, 'backup': {'name': f'{db_name}.sql.gz',
                                            'path': '/var/backups/databases/shop.sql.gz',
                                            'size': 4096}}

    monkeypatch.setattr(BackupService, 'backup_database', staticmethod(fake_backup_database))
    monkeypatch.setattr(BackupPolicyService, '_path_size', staticmethod(lambda p: 4096))

    with app.app_context():
        policy = BackupPolicyService.get_or_create_policy(
            'database', 11, target_subtype='mysql', target_meta={'db_name': 'shop'},
        )
        target = BackupPolicyService._resolve_target(policy)
        storage_path, size, meta = BackupPolicyService._execute_backup(policy, target, 'full')

    assert calls['db'][0] == 'mysql'
    assert calls['db'][1] == 'shop'
    assert storage_path.endswith('shop.sql.gz')
    assert size == 4096
    assert meta['includes'] == ['database']
    assert meta['kind'] == 'full'
    assert meta['primary_archive'] == storage_path


def test_execute_files_backup_routes_and_builds_meta(app, monkeypatch):
    from app.services.backup_service import BackupService
    calls = {}

    def fake_backup_files(file_paths, backup_name=None):
        calls['files'] = (list(file_paths), backup_name)
        return {'success': True, 'path': '/var/backups/files/configs_x.tar.gz',
                'backup': {'name': 'configs_x.tar.gz', 'size': 2048}}

    monkeypatch.setattr(BackupService, 'backup_files', staticmethod(fake_backup_files))

    with app.app_context():
        policy = BackupPolicyService.get_or_create_policy(
            'files', 12, target_meta={'paths': ['/etc/nginx'], 'label': 'configs'},
        )
        target = BackupPolicyService._resolve_target(policy)
        storage_path, size, meta = BackupPolicyService._execute_backup(policy, target, 'full')

    assert calls['files'][0] == ['/etc/nginx']
    assert storage_path.endswith('configs_x.tar.gz')
    assert size == 2048
    assert meta['includes'] == ['files']
    assert meta['paths'] == ['/etc/nginx']


def test_execute_database_backup_propagates_failure(app, monkeypatch):
    from app.services.backup_service import BackupService
    monkeypatch.setattr(BackupService, 'backup_database',
                        staticmethod(lambda **kw: {'success': False, 'error': 'mysqldump missing'}))
    with app.app_context():
        policy = BackupPolicyService.get_or_create_policy(
            'database', 13, target_subtype='mysql', target_meta={'db_name': 'x'})
        target = BackupPolicyService._resolve_target(policy)
        with pytest.raises(BackupPolicyError):
            BackupPolicyService._execute_backup(policy, target, 'full')
