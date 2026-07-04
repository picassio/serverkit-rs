"""Panel version resolution — the About page's version must track the real
install, including a custom SERVERKIT_DIR (rendered into the systemd unit as
SERVERKIT_INSTALL_DIR) and the Docker image layout, never a stale parallel
tree under the default /opt/serverkit.
"""
import os

import pytest

import app.utils.version as version_mod


@pytest.fixture(autouse=True)
def _reset_version_cache():
    """get_panel_version caches after first read — isolate every test."""
    version_mod._cached_version = None
    yield
    version_mod._cached_version = None


def _write_version(dirpath, value):
    path = dirpath / 'VERSION'
    path.write_text(value + '\n', encoding='utf-8')
    return path


def test_install_dir_override_wins(tmp_path, monkeypatch):
    """SERVERKIT_INSTALL_DIR (a custom install location) is honored first."""
    _write_version(tmp_path, '9.9.9-custom')
    monkeypatch.setenv('SERVERKIT_INSTALL_DIR', str(tmp_path))
    assert version_mod.get_panel_version() == '9.9.9-custom'


def test_override_without_version_file_falls_through(tmp_path, monkeypatch):
    """A pinned install dir with no VERSION file degrades to the own-tree read
    instead of reporting unknown."""
    monkeypatch.setenv('SERVERKIT_INSTALL_DIR', str(tmp_path))  # empty dir
    repo_root_version = os.path.join(
        os.path.dirname(os.path.abspath(version_mod.__file__)),
        '..', '..', '..', 'VERSION'
    )
    expected = open(repo_root_version, encoding='utf-8').read().strip()
    assert version_mod.get_panel_version() == expected


def test_own_tree_read_without_override(monkeypatch):
    """No env override: the running code's own tree (repo-root VERSION here,
    /app/VERSION in the Docker image) is the source — not /opt/serverkit."""
    monkeypatch.delenv('SERVERKIT_INSTALL_DIR', raising=False)
    repo_root_version = os.path.join(
        os.path.dirname(os.path.abspath(version_mod.__file__)),
        '..', '..', '..', 'VERSION'
    )
    expected = open(repo_root_version, encoding='utf-8').read().strip()
    got = version_mod.get_panel_version()
    assert got == expected
    assert got != '0.0.0'


def test_version_endpoint_reports_resolved_version(client, auth_headers, tmp_path, monkeypatch):
    """GET /api/v1/system/version (the About page's source) serves the
    resolver's answer, so a custom SERVERKIT_DIR box reports its real version."""
    _write_version(tmp_path, '8.8.8-endpoint')
    monkeypatch.setenv('SERVERKIT_INSTALL_DIR', str(tmp_path))
    version_mod._cached_version = None

    resp = client.get('/api/v1/system/version', headers=auth_headers)
    assert resp.status_code == 200
    data = resp.get_json()
    assert data['version'] == '8.8.8-endpoint'
    assert data['name'] == 'ServerKit'
    assert data['install_dir'] == str(tmp_path)


def test_install_dir_override(tmp_path, monkeypatch):
    """get_install_dir honors the explicit override (custom SERVERKIT_DIR)."""
    monkeypatch.setenv('SERVERKIT_INSTALL_DIR', str(tmp_path))
    assert version_mod.get_install_dir() == str(tmp_path)


def test_install_dir_own_tree_without_override(monkeypatch):
    """No override: the running code's own tree wins (it has a VERSION file
    here and in the Docker image) — never a hardcoded /opt/serverkit."""
    monkeypatch.delenv('SERVERKIT_INSTALL_DIR', raising=False)
    tree_root = os.path.abspath(os.path.join(
        os.path.dirname(os.path.abspath(version_mod.__file__)), '..', '..', '..'
    ))
    assert version_mod.get_install_dir() == tree_root
