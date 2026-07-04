"""§8 unification — BackupPolicy generalized to all data-protection targets.

Covers the additive model foundation (target_subtype + target_meta_json) and
target-type validation that lets a single BackupPolicy/BackupRun system back up
databases, file path-lists, and servers alongside apps and WordPress sites.
"""
import pytest

from app.models.backup_policy import BackupPolicy, VALID_TARGET_TYPES
from app.services.backup_policy_service import BackupPolicyService, BackupPolicyError


def test_valid_target_types_include_new_targets():
    for t in ('application', 'wordpress_site', 'database', 'files', 'server'):
        assert t in VALID_TARGET_TYPES


def test_target_meta_round_trips(app):
    with app.app_context():
        p = BackupPolicy(target_type='database', target_id=5, target_subtype='mysql')
        p.set_target_meta({'db_type': 'mysql', 'db_name': 'wp', 'host': 'localhost'})
        from app import db
        db.session.add(p)
        db.session.commit()

        reloaded = BackupPolicy.query.get(p.id)
        assert reloaded.target_subtype == 'mysql'
        assert reloaded.get_target_meta()['db_name'] == 'wp'
        d = reloaded.to_dict()
        assert d['target_subtype'] == 'mysql'
        assert d['target_meta']['db_type'] == 'mysql'


def test_get_target_meta_defaults_to_empty_dict(app):
    with app.app_context():
        p = BackupPolicy(target_type='files', target_id=1)
        assert p.get_target_meta() == {}


def test_validate_target_type_rejects_unknown():
    with pytest.raises(BackupPolicyError):
        BackupPolicyService.validate_target_type('nonsense')
    # Accepts every known type without raising.
    for t in VALID_TARGET_TYPES:
        BackupPolicyService.validate_target_type(t)


def test_get_or_create_persists_subtype_and_meta(app):
    with app.app_context():
        policy = BackupPolicyService.get_or_create_policy(
            'database', 42, target_subtype='postgresql',
            target_meta={'db_type': 'postgresql', 'db_name': 'app'},
        )
        assert policy.id is not None
        assert policy.target_subtype == 'postgresql'
        assert policy.get_target_meta()['db_name'] == 'app'

        # Second call updates descriptors on the same row (no duplicate).
        again = BackupPolicyService.get_or_create_policy(
            'database', 42, target_meta={'db_type': 'postgresql', 'db_name': 'renamed'},
        )
        assert again.id == policy.id
        assert again.get_target_meta()['db_name'] == 'renamed'


def test_get_or_create_rejects_bad_target_type(app):
    with app.app_context():
        with pytest.raises(BackupPolicyError):
            BackupPolicyService.get_or_create_policy('bogus', 1)
