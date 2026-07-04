"""§1 unification: /api/v1/services is a true alias of /api/v1/apps.

"Services" is the user-facing term for the backend's Applications. The same
`apps_bp` blueprint is mounted under both prefixes, so every route must resolve
identically regardless of which prefix a caller uses.
"""


def test_services_alias_matches_apps_list(client, auth_headers):
    apps_res = client.get('/api/v1/apps', headers=auth_headers)
    services_res = client.get('/api/v1/services', headers=auth_headers)

    assert apps_res.status_code == 200
    assert services_res.status_code == 200
    # Same handler, same (empty) data shape.
    assert services_res.get_json() == apps_res.get_json()


def test_services_alias_requires_auth_like_apps():
    # Both prefixes are JWT-protected; an unauthenticated call is rejected the
    # same way on either mount.
    import app as _app_pkg  # noqa: F401
    from app import create_app

    application = create_app('testing')
    with application.test_client() as c:
        apps_res = c.get('/api/v1/apps')
        services_res = c.get('/api/v1/services')
    assert apps_res.status_code == services_res.status_code
    assert apps_res.status_code in (401, 422)


def test_both_prefixes_registered_in_url_map():
    from app import create_app

    application = create_app('testing')
    rules = {r.rule for r in application.url_map.iter_rules()}
    assert '/api/v1/apps/' in rules or '/api/v1/apps' in rules
    assert '/api/v1/services/' in rules or '/api/v1/services' in rules
