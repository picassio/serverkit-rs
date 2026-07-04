"""§8 Phase B4 — /config-checkpoints aliases /snapshots.

"Config Checkpoint" is the user-facing name for a deployment config snapshot.
The snapshots blueprint serves every route under both /snapshots and
/config-checkpoints so the API matches the new wording while old callers keep
working.
"""


def test_config_checkpoints_resolves_same_handler(app, client, auth_headers):
    # Both prefixes hit the same view, so a missing app produces the identical
    # 404 response on either — proof the alias resolves to the same handler.
    snaps = client.get('/api/v1/apps/999999/snapshots', headers=auth_headers)
    ckpts = client.get('/api/v1/apps/999999/config-checkpoints', headers=auth_headers)
    assert snaps.status_code == 404
    assert ckpts.status_code == 404
    assert ckpts.get_json() == snaps.get_json()


def test_both_route_spaces_registered():
    from app import create_app
    application = create_app('testing')
    rules = {r.rule for r in application.url_map.iter_rules()}
    assert any('/snapshots' in r for r in rules)
    assert any('/config-checkpoints' in r for r in rules)
