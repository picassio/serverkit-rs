"""Tests for the deployment config snapshot + diff engine.

Covers the PURE core (hash stability, order-independence, diff detection, no
secret leakage) and the DB-backed snapshot lifecycle (dedupe, restore result).
"""

import json

import pytest

# Import the snapshot model at module load so its table is registered with
# SQLAlchemy metadata before the conftest `app` fixture runs create_all().
# (The parent agent wires this import into models/__init__.py globally; we
# import it directly here so our own tests are self-contained.)
import app.models.deployment_snapshot  # noqa: F401
from app.models.deployment_snapshot import DeploymentSnapshot
from app.services.configuration_service import ConfigurationService
from app.utils.sensitive_data_filter import MASK


# --------------------------------------------------------------------------- #
# Pure: hashing                                                               #
# --------------------------------------------------------------------------- #

def test_hash_config_is_stable():
    cfg = ConfigurationService.build_config(
        env=[{'key': 'A', 'value': '1', 'is_secret': False}],
        domains=['example.com'],
        image_tag='img:1',
        build_method='dockerfile',
        volumes=['data:/data'],
    )
    h1 = ConfigurationService.hash_config(cfg)
    h2 = ConfigurationService.hash_config(cfg)
    assert h1 == h2
    assert len(h1) == 64  # SHA-256 hex


def test_hash_config_is_order_independent():
    cfg_a = ConfigurationService.build_config(
        env=[
            {'key': 'A', 'value': '1', 'is_secret': False},
            {'key': 'B', 'value': '2', 'is_secret': False},
        ],
        domains=['a.com', 'b.com'],
    )
    cfg_b = ConfigurationService.build_config(
        env=[
            {'key': 'B', 'value': '2', 'is_secret': False},
            {'key': 'A', 'value': '1', 'is_secret': False},
        ],
        domains=['b.com', 'a.com'],
    )
    assert ConfigurationService.hash_config(cfg_a) == ConfigurationService.hash_config(cfg_b)


def test_hash_changes_when_value_changes():
    base = ConfigurationService.build_config(
        env=[{'key': 'A', 'value': '1', 'is_secret': False}]
    )
    changed = ConfigurationService.build_config(
        env=[{'key': 'A', 'value': '2', 'is_secret': False}]
    )
    assert ConfigurationService.hash_config(base) != ConfigurationService.hash_config(changed)


# --------------------------------------------------------------------------- #
# Pure: masking                                                               #
# --------------------------------------------------------------------------- #

def test_secret_values_are_masked_in_config():
    cfg = ConfigurationService.build_config(
        env=[
            {'key': 'PUBLIC', 'value': 'visible', 'is_secret': False},
            {'key': 'API_TOKEN', 'value': 'super-secret', 'is_secret': True},
        ]
    )
    assert cfg['env']['PUBLIC'] == 'visible'
    assert cfg['env']['API_TOKEN'] == MASK
    # The raw secret must never appear anywhere in the serialized config.
    import json
    assert 'super-secret' not in json.dumps(cfg)


def test_sensitive_key_name_is_masked_even_without_flag():
    # Key *name* looks sensitive → masked even if is_secret is False.
    cfg = ConfigurationService.build_config(
        env=[{'key': 'DB_PASSWORD', 'value': 'hunter2', 'is_secret': False}]
    )
    assert cfg['env']['DB_PASSWORD'] == MASK
    import json
    assert 'hunter2' not in json.dumps(cfg)


# --------------------------------------------------------------------------- #
# Pure: diffing                                                               #
# --------------------------------------------------------------------------- #

def test_diff_detects_env_add_remove_change():
    old = ConfigurationService.build_config(env=[
        {'key': 'KEEP', 'value': 'same', 'is_secret': False},
        {'key': 'GONE', 'value': 'x', 'is_secret': False},
        {'key': 'CHANGED', 'value': 'old', 'is_secret': False},
    ])
    new = ConfigurationService.build_config(env=[
        {'key': 'KEEP', 'value': 'same', 'is_secret': False},
        {'key': 'NEW', 'value': 'y', 'is_secret': False},
        {'key': 'CHANGED', 'value': 'new', 'is_secret': False},
    ])
    diff = ConfigurationService.diff_configs(old, new)
    assert diff['env']['added'] == ['NEW']
    assert diff['env']['removed'] == ['GONE']
    assert diff['env']['changed'] == ['CHANGED']
    assert ConfigurationService.has_changes(diff)


def test_diff_detects_image_and_domain_change():
    old = ConfigurationService.build_config(
        image_tag='img:1', domains=['a.com'], build_method='dockerfile'
    )
    new = ConfigurationService.build_config(
        image_tag='img:2', domains=['a.com', 'b.com'], build_method='nixpacks'
    )
    diff = ConfigurationService.diff_configs(old, new)
    assert diff['image']['changed'] is True
    assert diff['image']['old'] == 'img:1'
    assert diff['image']['new'] == 'img:2'
    assert diff['domains']['added'] == ['b.com']
    assert diff['build_method']['changed'] is True


def test_diff_never_leaks_secret_values():
    old = ConfigurationService.build_config(
        env=[{'key': 'API_TOKEN', 'value': 'old-secret', 'is_secret': True}]
    )
    new = ConfigurationService.build_config(
        env=[{'key': 'API_TOKEN', 'value': 'new-secret', 'is_secret': True}]
    )
    diff = ConfigurationService.diff_configs(old, new)
    import json
    serialized = json.dumps(diff)
    assert 'old-secret' not in serialized
    assert 'new-secret' not in serialized
    # Both masked to the same MASK, so the key is NOT reported as changed
    # (we cannot tell — and must not leak). That's the safe behavior.
    assert 'API_TOKEN' not in diff['env']['changed']


def test_no_change_diff_is_empty():
    cfg = ConfigurationService.build_config(
        env=[{'key': 'A', 'value': '1', 'is_secret': False}], image_tag='img:1'
    )
    diff = ConfigurationService.diff_configs(cfg, cfg)
    assert ConfigurationService.has_changes(diff) is False
    assert ConfigurationService.summarize_diff(diff) == 'no config changes'


def test_summarize_diff_is_a_human_sentence_with_image_tag():
    old = ConfigurationService.build_config(
        env=[
            {'key': 'A', 'value': '1', 'is_secret': False},
            {'key': 'B', 'value': '1', 'is_secret': False},
        ],
        image_tag='app:1.2.1',
    )
    new = ConfigurationService.build_config(
        env=[
            {'key': 'A', 'value': '2', 'is_secret': False},
            {'key': 'B', 'value': '2', 'is_secret': False},
            {'key': 'C', 'value': '3', 'is_secret': False},
        ],
        image_tag='app:1.1.9',
    )
    diff = ConfigurationService.diff_configs(old, new)
    summary = ConfigurationService.summarize_diff(diff)

    # Reads as a sentence, names the image tag transition, and is bounded.
    assert summary == (
        '3 environment variables and the image tag '
        '(app:1.2.1 → app:1.1.9) changed'
    )
    assert len(summary) <= 255
    # Backward-compatible: still mentions env vars and the image.
    assert 'environment variable' in summary
    assert 'image tag' in summary


def test_summarize_diff_single_env_var_is_singular():
    old = ConfigurationService.build_config(
        env=[{'key': 'A', 'value': '1', 'is_secret': False}]
    )
    new = ConfigurationService.build_config(
        env=[{'key': 'A', 'value': '2', 'is_secret': False}]
    )
    summary = ConfigurationService.summarize_diff(
        ConfigurationService.diff_configs(old, new)
    )
    assert summary == '1 environment variable changed'


# --------------------------------------------------------------------------- #
# DB-backed: snapshot lifecycle                                               #
# --------------------------------------------------------------------------- #

def _make_app(db):
    from app.models.application import Application
    from app.models.user import User
    from werkzeug.security import generate_password_hash

    user = User(
        email='snaptest@test.local', username='snaptest',
        password_hash=generate_password_hash('x'), role=User.ROLE_ADMIN,
        is_active=True,
    )
    db.session.add(user)
    db.session.commit()

    app_row = Application(
        name='snap-app', app_type='docker', user_id=user.id,
        docker_image='img:1', root_path='/tmp/nonexistent-snap-app',
    )
    db.session.add(app_row)
    db.session.commit()
    return app_row, user


def test_create_snapshot_dedupes_identical_config(app):
    # The `app` fixture already runs inside an app_context with create_all().
    from app import db

    app_row, _user = _make_app(db)

    snap1 = ConfigurationService.create_snapshot(app_row)
    snap2 = ConfigurationService.create_snapshot(app_row)

    # Identical config → same row returned, no duplicate.
    assert snap1.id == snap2.id
    assert DeploymentSnapshot.query.filter_by(
        application_id=app_row.id
    ).count() == 1

    # Mutate config → a new snapshot is created.
    app_row.docker_image = 'img:2'
    db.session.commit()
    snap3 = ConfigurationService.create_snapshot(app_row)
    assert snap3.id != snap1.id
    assert DeploymentSnapshot.query.filter_by(
        application_id=app_row.id
    ).count() == 2
    assert snap3.snapshot_hash != snap1.snapshot_hash


def test_snapshot_config_masks_secret_env(app):
    from app import db
    from app.services.env_service import EnvService

    app_row, user = _make_app(db)

    EnvService.set_env_var(app_row.id, 'PUBLIC', 'visible', is_secret=False, user_id=user.id)
    EnvService.set_env_var(app_row.id, 'SECRET_TOKEN', 'topsecret', is_secret=True, user_id=user.id)

    snap = ConfigurationService.create_snapshot(app_row)
    config = snap.get_config()
    assert config['env']['PUBLIC'] == 'visible'
    assert config['env']['SECRET_TOKEN'] == MASK
    assert 'topsecret' not in json.dumps(config)


def test_restore_snapshot_returns_sensible_result(app):
    from app import db
    from app.services.env_service import EnvService

    app_row, user = _make_app(db)

    EnvService.set_env_var(app_row.id, 'FEATURE_FLAG', 'on', is_secret=False, user_id=user.id)
    EnvService.set_env_var(app_row.id, 'SECRET_TOKEN', 'topsecret', is_secret=True, user_id=user.id)

    snap = ConfigurationService.create_snapshot(app_row)

    # Change the value, then restore the snapshot.
    EnvService.set_env_var(app_row.id, 'FEATURE_FLAG', 'off', is_secret=False, user_id=user.id)

    result = ConfigurationService.restore_snapshot(snap.id, user_id=user.id)
    assert result['success'] is True
    assert 'FEATURE_FLAG' in result['restored']['env']
    # Masked secret is skipped (not overwritten with the mask placeholder).
    assert 'SECRET_TOKEN' in result['skipped_secrets']

    # The non-secret value was restored to the snapshot's value.
    restored = EnvService.get_env_var(app_row.id, 'FEATURE_FLAG')
    assert restored.value == 'on'
    # The real secret value is untouched.
    secret = EnvService.get_env_var(app_row.id, 'SECRET_TOKEN')
    assert secret.value == 'topsecret'


def test_restore_missing_snapshot_returns_error(app):
    result = ConfigurationService.restore_snapshot(999999)
    assert result['success'] is False
    assert 'error' in result
