import pytest


def test_list_plugins_empty(http, base_url, auth_headers):
    r = http.get(f"{base_url}/api/v1/plugins", headers=auth_headers, timeout=15)
    assert r.status_code == 200, r.text
    body = r.json()
    # accept either {"plugins": [...]} or a bare list
    if isinstance(body, dict):
        assert "plugins" in body
    else:
        assert isinstance(body, list)


@pytest.mark.skipif(
    True,
    reason="install-from-URL hits GitHub; enable when a stable test plugin repo exists",
)
def test_install_plugin_from_url(http, base_url, auth_headers):
    # Placeholder — wire to a known-good test plugin repo once one exists.
    r = http.post(
        f"{base_url}/api/v1/plugins/install",
        headers=auth_headers,
        json={"url": "https://github.com/jhd3197/serverkit-gui"},
        timeout=120,
    )
    assert r.status_code in (200, 201), r.text
    plugin = r.json()
    pid = plugin["id"]

    # disable
    r = http.post(
        f"{base_url}/api/v1/plugins/{pid}/disable", headers=auth_headers, timeout=15
    )
    assert r.status_code == 200, r.text

    # enable
    r = http.post(
        f"{base_url}/api/v1/plugins/{pid}/enable", headers=auth_headers, timeout=15
    )
    assert r.status_code == 200, r.text

    # uninstall
    r = http.delete(
        f"{base_url}/api/v1/plugins/{pid}", headers=auth_headers, timeout=30
    )
    assert r.status_code in (200, 204), r.text
