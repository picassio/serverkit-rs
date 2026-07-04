"""§4 unification — one /api/v1/observability namespace.

The monitoring / metrics / telemetry(events) / uptime / fleet / status-page read
surfaces are re-mounted under /api/v1/observability/<domain> as true aliases.
The original prefixes and the public status page route stay intact.
"""


def test_observability_aliases_registered():
    from app import create_app
    application = create_app('testing')
    rules = {r.rule for r in application.url_map.iter_rules()}
    assert any(r.startswith('/api/v1/observability/monitoring') for r in rules)
    assert any(r.startswith('/api/v1/observability/metrics') for r in rules)
    assert any(r.startswith('/api/v1/observability/events') for r in rules)
    assert any(r.startswith('/api/v1/observability/uptime') for r in rules)
    assert any(r.startswith('/api/v1/observability/fleet') for r in rules)
    assert any(r.startswith('/api/v1/observability/status-pages') for r in rules)


def test_original_prefixes_still_present():
    from app import create_app
    application = create_app('testing')
    rules = {r.rule for r in application.url_map.iter_rules()}
    assert any(r.startswith('/api/v1/monitoring') for r in rules)
    assert any(r.startswith('/api/v1/telemetry') for r in rules)
    assert any(r.startswith('/api/v1/fleet-monitor') for r in rules)
    # Public status page route untouched.
    assert '/api/v1/status/public/<slug>' in rules


def test_events_alias_matches_telemetry(app, client, auth_headers):
    canonical = client.get('/api/v1/telemetry/events', headers=auth_headers)
    alias = client.get('/api/v1/observability/events/events', headers=auth_headers)
    assert canonical.status_code == 200
    assert alias.status_code == 200
    assert alias.get_json() == canonical.get_json()
