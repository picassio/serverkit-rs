"""Module toggles (#14): the generic core-vertical toggle machinery.

Email (#32) and WordPress (#38) both graduated from module toggles to bundled
extensions (serverkit-email / serverkit-wordpress), so the MODULES map is now
empty. Their APIs are gated by the plugin status guard instead — proven in
test_wordpress_extraction.py / test_email_extraction.py. These tests keep the
generic toggle machinery honest so a future core-vertical toggle can be added
without re-plumbing.
"""
from app.services import module_service


def test_modules_default_empty(app, client, auth_headers):
    resp = client.get('/api/v1/modules', headers=auth_headers)
    assert resp.status_code == 200
    mods = {m['name']: m for m in resp.get_json()['modules']}
    # WordPress and Email are extensions now, not module toggles.
    assert 'wordpress' not in mods
    assert 'email' not in mods


def test_unknown_module_404(app, client, auth_headers):
    resp = client.put('/api/v1/modules/nope', headers=auth_headers, json={'enabled': False})
    assert resp.status_code == 404


def test_wordpress_api_reachable_via_extension(app, client, auth_headers):
    """WordPress ships as a default-installed flagship extension, so its API is
    reachable (not 503) on a stock panel — no module toggle stands in front."""
    resp = client.get('/api/v1/wordpress/sites', headers=auth_headers)
    assert resp.status_code != 503


def test_is_module_enabled_fails_open(app):
    # Unknown module name is treated as enabled (never hide a core feature).
    assert module_service.is_module_enabled('does-not-exist') is True
