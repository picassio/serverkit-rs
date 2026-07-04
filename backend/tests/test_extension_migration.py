"""Prove the one-shot auto-install of converted builtin extensions (D3).

An *upgraded* panel (has users) auto-installs a converted builtin once; a *fresh*
panel (no users yet) does not — it sees the extension in the Marketplace. Either
way the pass is idempotent and recorded so it never repeats.
"""
import json

import pytest
from werkzeug.security import generate_password_hash

from app import db
from app.models.plugin import InstalledPlugin
from app.models.user import User
from app.services import plugin_service, extension_migration


@pytest.fixture
def demo_builtin(tmp_path, monkeypatch):
    """Redirect plugin dirs to temp, ship one converted builtin ('serverkit-demo')."""
    backend = tmp_path / 'backend_plugins'
    frontend = tmp_path / 'frontend_plugins'
    builtin = tmp_path / 'builtin_extensions'
    for d in (backend, frontend, builtin):
        d.mkdir()
    monkeypatch.setattr(plugin_service, 'BACKEND_PLUGINS_DIR', str(backend))
    monkeypatch.setattr(plugin_service, 'FRONTEND_PLUGINS_DIR', str(frontend))
    monkeypatch.setattr(plugin_service, 'BUILTIN_EXTENSIONS_DIR', str(builtin))
    monkeypatch.setattr(extension_migration, 'CONVERTED_BUILTIN_SLUGS', ['serverkit-demo'])

    folder = builtin / 'serverkit-demo'
    (folder / 'frontend').mkdir(parents=True)
    manifest = {
        'name': 'serverkit-demo', 'display_name': 'Demo', 'version': '1.0.0',
        'category': 'utility',
        'contributions': {'nav': [{'id': 'demo', 'label': 'Demo', 'route': '/demo'}]},
    }
    (folder / 'plugin.json').write_text(json.dumps(manifest), encoding='utf-8')
    (folder / 'frontend' / 'index.jsx').write_text('export function P(){return null;}\n')
    return builtin


def _make_user():
    u = User(email='u@test.local', username='someuser',
             password_hash=generate_password_hash('x'),
             role=User.ROLE_ADMIN, is_active=True)
    db.session.add(u)
    db.session.commit()


def test_upgrade_auto_installs_converted_builtin(app, demo_builtin):
    _make_user()  # existing install
    extension_migration.run_auto_install()

    p = InstalledPlugin.query.filter_by(slug='serverkit-demo').first()
    assert p is not None
    assert p.status == InstalledPlugin.STATUS_ACTIVE

    # Marker recorded → second run is a no-op (no error, still one row).
    extension_migration.run_auto_install()
    assert InstalledPlugin.query.filter_by(slug='serverkit-demo').count() == 1


def test_fresh_install_does_not_auto_install(app, demo_builtin):
    # No users → brand-new panel.
    extension_migration.run_auto_install()
    assert InstalledPlugin.query.filter_by(slug='serverkit-demo').first() is None
    # But it was marked processed, so a later boot (even once users exist) won't
    # retroactively install it.
    assert 'serverkit-demo' in extension_migration._processed_slugs()


def test_user_uninstall_is_not_undone(app, demo_builtin):
    _make_user()
    extension_migration.run_auto_install()
    p = InstalledPlugin.query.filter_by(slug='serverkit-demo').first()
    plugin_service.uninstall_plugin(p.id)
    assert InstalledPlugin.query.filter_by(slug='serverkit-demo').first() is None

    # Re-running must NOT reinstall — the one-shot already happened.
    extension_migration.run_auto_install()
    assert InstalledPlugin.query.filter_by(slug='serverkit-demo').first() is None
