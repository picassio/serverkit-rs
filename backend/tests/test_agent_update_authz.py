"""
Authorization regression test for the agent self-update endpoint.

POST /api/v1/servers/<id>/agent/update replaces the agent binary and restarts
the service. It used to be @jwt_required() only, so a read-only `viewer` could
trigger fleet-wide updates/restarts — every other state-changing server route is
@developer_required. This locks it to developer+ and proves both directions.
"""
from flask_jwt_extended import create_access_token
from werkzeug.security import generate_password_hash

from app import db
from app.models import User
from app.models.server import Server


def _user(role):
    u = User(
        email=f"{role}@authz.local",
        username=f"{role}-authz",
        password_hash=generate_password_hash("x"),
        role=role,
        is_active=True,
    )
    db.session.add(u)
    db.session.commit()
    return u


def _headers(user):
    return {"Authorization": f"Bearer {create_access_token(identity=user.id)}"}


def _server():
    s = Server(name="authz-srv", agent_id="authz-agent")
    db.session.add(s)
    db.session.commit()
    return s.id


def test_viewer_cannot_trigger_agent_update(client, app):
    with app.app_context():
        headers = _headers(_user(User.ROLE_VIEWER))
        server_id = _server()

    resp = client.post(f"/api/v1/servers/{server_id}/agent/update",
                       headers=headers, json={})
    assert resp.status_code == 403, resp.get_data(as_text=True)


def test_developer_passes_the_role_gate(client, app):
    """A developer clears the role gate. The request then fails downstream
    (503, no agent connected) — which is exactly what proves the gate let them
    through rather than blocking with 403."""
    with app.app_context():
        headers = _headers(_user(User.ROLE_DEVELOPER))
        server_id = _server()

    resp = client.post(f"/api/v1/servers/{server_id}/agent/update",
                       headers=headers, json={})
    assert resp.status_code != 403, resp.get_data(as_text=True)
    assert resp.status_code == 503, resp.get_data(as_text=True)  # agent offline
