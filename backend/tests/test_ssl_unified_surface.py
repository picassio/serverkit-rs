"""§5 unification — one /ssl surface.

The advanced certificate operations (wildcard, SAN, upload, profiles, health,
expiry alerts) are mounted under /api/v1/ssl alongside the basic certbot routes,
with /api/v1/ssl/advanced kept as a deprecated alias. Both resolve to the same
handlers.
"""


def test_advanced_routes_mounted_under_ssl():
    from app import create_app
    application = create_app('testing')
    rules = {r.rule for r in application.url_map.iter_rules()}
    # Unified surface
    assert '/api/v1/ssl/profiles' in rules
    assert '/api/v1/ssl/wildcard' in rules
    assert '/api/v1/ssl/upload' in rules
    assert '/api/v1/ssl/expiry-alerts' in rules
    # Deprecated alias still present
    assert '/api/v1/ssl/advanced/profiles' in rules


def test_basic_ssl_routes_still_present():
    from app import create_app
    application = create_app('testing')
    rules = {r.rule for r in application.url_map.iter_rules()}
    assert '/api/v1/ssl/certificates' in rules
    assert '/api/v1/ssl/status' in rules


def test_profiles_resolves_on_both_prefixes(app, client, auth_headers):
    unified = client.get('/api/v1/ssl/profiles', headers=auth_headers)
    alias = client.get('/api/v1/ssl/advanced/profiles', headers=auth_headers)
    assert unified.status_code == 200
    assert alias.status_code == 200
    assert unified.get_json() == alias.get_json()
