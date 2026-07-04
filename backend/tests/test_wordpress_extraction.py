"""Prove the WordPress extraction (Phase 5, #38).

WordPress physically moved out of core into the bundled, default-installed
``serverkit-wordpress`` extension (a flagship, D4). Unlike a niche extension it
ships installed on every panel, so the proof is the mirror of the Email one:

* the WordPress *services* are gone from ``app.services`` (physical move);
* they load instead from the dashed package ``app.plugins.serverkit-wordpress``
  via the importlib bridge — in-place, no copy (dev/test loadability);
* a stock panel has the ``/api/v1/wordpress`` blueprints registered *from the
  extension* (seeded flagship), including the ``/pipelines`` alias and
  ``/projects`` mounts (D9);
* uninstall records the user's choice and NEVER deletes the tracked builtin /
  pre-bundled files.
"""
import importlib
import os

import pytest

from app.models.plugin import InstalledPlugin
from app.services import plugin_service, wordpress_bridge

SLUG = 'serverkit-wordpress'


def test_wordpress_services_left_core():
    """The WordPress service files no longer exist under app.services."""
    for mod in ('wordpress_service', 'git_wordpress_service',
                'wordpress_env_service', 'wp_update_service'):
        with pytest.raises(ModuleNotFoundError):
            importlib.import_module(f'app.services.{mod}')


def test_core_services_pkg_drops_wordpress_export():
    import app.services as services_pkg
    assert 'WordPressService' not in getattr(services_pkg, '__all__', [])
    assert not hasattr(services_pkg, 'WordPressService')


def test_bridge_resolves_from_extension_in_place():
    """The bridge loads WordPressService from the dashed extension package,
    without the plugin being copy-installed under app/plugins/."""
    WordPressService = wordpress_bridge.wordpress_service()
    assert WordPressService.__name__ == 'WordPressService'
    assert WordPressService.__module__.startswith(f'app.plugins.{SLUG}')


def test_wordpress_seeded_as_flagship(app):
    """A stock panel ships WordPress installed-by-default (D4)."""
    plugin = InstalledPlugin.query.filter_by(slug=SLUG).first()
    assert plugin is not None, 'WordPress flagship should be seeded on boot'
    assert plugin.status == InstalledPlugin.STATUS_ACTIVE
    assert plugin.has_backend is True
    assert plugin.url_prefix == '/api/v1/wordpress'


def test_wordpress_blueprints_registered_from_extension(app):
    rules = [r.rule for r in app.url_map.iter_rules()]
    wp_rules = [r for r in rules if r.startswith('/api/v1/wordpress')]
    assert wp_rules, 'WordPress API should be registered from the extension'
    # D9: the /projects and /pipelines (alias) mounts both survive the move.
    assert any(r.startswith('/api/v1/wordpress/projects') for r in wp_rules)
    assert any(r.startswith('/api/v1/wordpress/pipelines') for r in wp_rules)


def test_wordpress_api_responds_not_404_or_503(app, client, auth_headers):
    """The extension's blueprint is live: /sites is wired (not 404) and its
    status guard passes for the active flagship (not 503)."""
    resp = client.get('/api/v1/wordpress/sites', headers=auth_headers)
    assert resp.status_code not in (404, 503), resp.status_code


def test_uninstall_flagship_keeps_tracked_files_and_marks(app):
    plugin = InstalledPlugin.query.filter_by(slug=SLUG).first()
    assert plugin is not None
    builtin_init = os.path.join(
        plugin_service.BUILTIN_EXTENSIONS_DIR, SLUG, 'backend', '__init__.py')
    assert os.path.isfile(builtin_init)

    assert plugin_service.uninstall_plugin(plugin.id) is True
    # Row gone, marker set, and the tracked builtin source is untouched.
    assert InstalledPlugin.query.filter_by(slug=SLUG).first() is None
    assert SLUG in plugin_service._flagship_uninstalled_set()
    assert os.path.isfile(builtin_init), 'uninstall must not delete builtin files'
