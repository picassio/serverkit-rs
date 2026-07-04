"""Registry fetch/cache, checksum-verified install, version gates, update flow."""
import hashlib
import io
import json
import zipfile

import pytest

from app import db
from app.models.plugin import InstalledPlugin
from app.services import plugin_service, registry_service
from app.utils import version as version_util


@pytest.fixture(autouse=True)
def _reset_registry_cache():
    registry_service._cache.update({'ts': 0.0, 'entries': None, 'source': None})
    yield
    registry_service._cache.update({'ts': 0.0, 'entries': None, 'source': None})


@pytest.fixture
def plugin_dirs(tmp_path, monkeypatch):
    backend = tmp_path / 'b'
    frontend = tmp_path / 'f'
    for d in (backend, frontend):
        d.mkdir()
    monkeypatch.setattr(plugin_service, 'BACKEND_PLUGINS_DIR', str(backend))
    monkeypatch.setattr(plugin_service, 'FRONTEND_PLUGINS_DIR', str(frontend))
    return {'backend': backend, 'frontend': frontend}


def _make_plugin_zip(slug='regext', version='2.0.0'):
    """A minimal, valid backend-less plugin zip (frontend-only)."""
    manifest = {
        'name': slug, 'display_name': 'Registry Ext', 'version': version,
        'category': 'utility',
        'contributions': {'nav': [{'id': slug, 'label': 'Reg', 'route': '/reg'}]},
    }
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, 'w') as zf:
        zf.writestr('plugin.json', json.dumps(manifest))
        zf.writestr('frontend/index.jsx', 'export function P(){return null;}\n')
    return buf.getvalue()


# --------------------------------------------------------------------------- #
# Registry fetch / cache (offline-tolerant → bundled fallback)
# --------------------------------------------------------------------------- #

def test_bundled_registry_lists_serverkit_gui(app):
    # No SERVERKIT_REGISTRY_URL in tests → bundled index is used.
    entries = registry_service.list_extensions()
    slugs = {e['slug'] for e in entries}
    assert 'serverkit-gui' in slugs
    assert registry_service.registry_source_label() == 'bundled'


def test_registry_catalog_carries_install_state(app):
    catalog = registry_service.list_catalog()
    gui = next(e for e in catalog if e['slug'] == 'serverkit-gui')
    assert gui['installed'] is False
    assert gui['status'] == 'not_installed'
    assert gui['source_kind'] == 'registry'


def test_registry_endpoint(app, client, auth_headers):
    resp = client.get('/api/v1/marketplace/registry', headers=auth_headers)
    assert resp.status_code == 200
    data = resp.get_json()
    assert any(e['slug'] == 'serverkit-gui' for e in data['extensions'])


# --------------------------------------------------------------------------- #
# Version helpers + compat gates
# --------------------------------------------------------------------------- #

def test_version_helpers():
    assert version_util.compare_versions('2.0.0', '1.9.9') == 1
    assert version_util.compare_versions('1.0.0', '1.0.0') == 0
    assert version_util.version_satisfies('1.7.5', min_version='1.7.0') is True
    assert version_util.version_satisfies('1.6.0', min_version='1.7.0') is False
    assert version_util.version_satisfies('2.5.0', max_version='2.0.0') is False


def test_registry_install_blocked_by_min_panel_version(app, plugin_dirs, monkeypatch):
    monkeypatch.setattr(registry_service, '_cache', {
        'ts': 9e18, 'source': 'test',
        'entries': [registry_service._normalize({
            'slug': 'future-ext', 'display_name': 'Future', 'version': '1.0.0',
            'source': 'https://example.com/x.zip', 'min_panel_version': '999.0.0',
        })],
    })
    with pytest.raises(ValueError, match='needs panel'):
        plugin_service.install_registry_extension('future-ext')


# --------------------------------------------------------------------------- #
# Checksum-verified install
# --------------------------------------------------------------------------- #

def test_checksum_mismatch_rejected(app, plugin_dirs, monkeypatch):
    zip_bytes = _make_plugin_zip()
    monkeypatch.setattr(plugin_service, '_download_zip', lambda url: io.BytesIO(zip_bytes))

    with pytest.raises(ValueError, match='Checksum mismatch'):
        plugin_service.install_from_url('https://x/y.zip', expected_sha256='deadbeef')

    # Nothing was installed.
    assert InstalledPlugin.query.filter_by(slug='regext').first() is None


def test_checksum_match_installs(app, plugin_dirs, monkeypatch):
    zip_bytes = _make_plugin_zip()
    digest = hashlib.sha256(zip_bytes).hexdigest()
    monkeypatch.setattr(plugin_service, '_download_zip', lambda url: io.BytesIO(zip_bytes))

    plugin = plugin_service.install_from_url('https://x/y.zip', expected_sha256=digest)
    assert plugin.status == InstalledPlugin.STATUS_ACTIVE
    assert plugin.version == '2.0.0'


# --------------------------------------------------------------------------- #
# Update flow
# --------------------------------------------------------------------------- #

def test_update_flow(app, plugin_dirs, monkeypatch):
    # Install v1.0.0 first.
    v1 = _make_plugin_zip(slug='regext', version='1.0.0')
    monkeypatch.setattr(plugin_service, '_download_zip', lambda url: io.BytesIO(v1))
    plugin = plugin_service.install_from_url('https://x/regext.zip')
    assert plugin.version == '1.0.0'

    # Registry advertises v2.0.0 for the same slug.
    monkeypatch.setattr(registry_service, '_cache', {
        'ts': 9e18, 'source': 'test',
        'entries': [registry_service._normalize({
            'slug': 'regext', 'display_name': 'Registry Ext', 'version': '2.0.0',
            'source': 'https://x/regext.zip', 'min_panel_version': '0.0.1',
        })],
    })

    updates = {u['slug']: u for u in plugin_service.check_for_updates()}
    assert updates['regext']['update_available'] is True
    assert updates['regext']['available_version'] == '2.0.0'
    assert updates['regext']['compatible'] is True

    # Serve v2 bytes and run the update (reinstall over active → force).
    v2 = _make_plugin_zip(slug='regext', version='2.0.0')
    monkeypatch.setattr(plugin_service, '_download_zip', lambda url: io.BytesIO(v2))
    updated = plugin_service.update_plugin(plugin.id)
    assert updated.version == '2.0.0'
    assert InstalledPlugin.query.filter_by(slug='regext').count() == 1
