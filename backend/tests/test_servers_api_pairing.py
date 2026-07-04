"""End-to-end tests for the connection-string pairing flow.

Covers:
- Creating a server with no name returns a placeholder + connection string.
- Registering with a hostname replaces the placeholder.
- A second register (re-pair) does NOT clobber a user-chosen name.
- regenerate-token produces a new connection string.
"""

from app.services import connection_string as cs
from app.models.server import Server
from app import db


def _create_server(client, auth_headers, expires_in=None):
    body = {}
    if expires_in is not None:
        body['expires_in'] = expires_in
    return client.post('/api/v1/servers', headers=auth_headers, json=body)


def test_create_server_without_name_returns_connection_string(client, auth_headers, app):
    resp = _create_server(client, auth_headers)
    assert resp.status_code == 201, resp.get_json()

    data = resp.get_json()
    assert 'connection_string' in data
    assert data['connection_string'].startswith('sk1://')
    assert data['name'].startswith('Pending pairing (')

    # Token in the connection string matches the registration_token in the response
    decoded = cs.decode(data['connection_string'])
    assert decoded['token'] == data['registration_token']
    assert decoded['url']  # whatever the test client reports as host
    assert decoded['expires_at'] is not None  # default 7d, not "never"


def test_create_server_with_never_expiry(client, auth_headers, app):
    resp = _create_server(client, auth_headers, expires_in=-1)
    assert resp.status_code == 201
    decoded = cs.decode(resp.get_json()['connection_string'])
    # 100 years out is the "never" sentinel — far enough in the future
    # that any sane "is this still valid" check passes.
    assert decoded['expires_at'].year >= 2100


def test_register_replaces_placeholder_with_hostname(client, auth_headers, app):
    create_resp = _create_server(client, auth_headers)
    server_id = create_resp.get_json()['id']
    token = create_resp.get_json()['registration_token']

    register_resp = client.post('/api/v1/servers/register', json={
        'token': token,
        'system_info': {'hostname': 'web-01.prod', 'os': 'linux'},
        'agent_version': '1.0.0',
    })
    assert register_resp.status_code == 200, register_resp.get_json()

    with app.app_context():
        server = Server.query.get(server_id)
        assert server.name == 'web-01.prod'
        assert server.hostname == 'web-01.prod'


def test_re_register_does_not_overwrite_user_chosen_name(client, auth_headers, app):
    """If the user renamed the server in the UI before the agent reinstalls,
    the second register must not overwrite that name."""
    create_resp = _create_server(client, auth_headers)
    server_id = create_resp.get_json()['id']
    token1 = create_resp.get_json()['registration_token']

    # First pair: placeholder → hostname
    client.post('/api/v1/servers/register', json={
        'token': token1,
        'system_info': {'hostname': 'web-01'},
    })

    # User renames in the UI (simulated directly on the model)
    with app.app_context():
        server = Server.query.get(server_id)
        server.name = 'My Production Web Server'
        db.session.commit()

    # Regenerate token + re-pair (agent reinstalled)
    regen_resp = client.post(
        f'/api/v1/servers/{server_id}/regenerate-token',
        headers=auth_headers,
        json={},
    )
    assert regen_resp.status_code == 200
    token2 = regen_resp.get_json()['registration_token']
    assert 'connection_string' in regen_resp.get_json()

    client.post('/api/v1/servers/register', json={
        'token': token2,
        'system_info': {'hostname': 'web-01-new-hostname'},
    })

    with app.app_context():
        server = Server.query.get(server_id)
        assert server.name == 'My Production Web Server'
        # Hostname still updates — that field tracks reality, name is for humans
        assert server.hostname == 'web-01-new-hostname'


def test_regenerate_token_returns_connection_string(client, auth_headers, app):
    create_resp = _create_server(client, auth_headers)
    server_id = create_resp.get_json()['id']
    original_token = create_resp.get_json()['registration_token']

    regen_resp = client.post(
        f'/api/v1/servers/{server_id}/regenerate-token',
        headers=auth_headers,
        json={'expires_in': 3600},  # 1 hour
    )
    assert regen_resp.status_code == 200
    body = regen_resp.get_json()
    assert body['connection_string'].startswith('sk1://')

    new_token = body['registration_token']
    assert new_token != original_token

    decoded = cs.decode(body['connection_string'])
    assert decoded['token'] == new_token
