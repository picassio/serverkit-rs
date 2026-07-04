"""
Authorization test for audit L1: pairing claim/lookup create or reveal fleet
servers, so they should require developer role for consistency with
POST /api/v1/servers (@developer_required) — not just any authenticated user.
"""
from flask_jwt_extended import create_access_token
from werkzeug.security import generate_password_hash

from app import db
from app.models import User


def _user(role):
    u = User(
        email=f"{role}@pair.local",
        username=f"{role}-pair",
        password_hash=generate_password_hash("x"),
        role=role,
        is_active=True,
    )
    db.session.add(u)
    db.session.commit()
    return u


def _headers(user):
    return {"Authorization": f"Bearer {create_access_token(identity=user.id)}"}


def test_viewer_cannot_lookup_or_claim(client, app):
    with app.app_context():
        headers = _headers(_user(User.ROLE_VIEWER))

    r1 = client.post("/api/v1/pairing/lookup", headers=headers, json={"code": "ABC123"})
    assert r1.status_code == 403, r1.get_data(as_text=True)

    r2 = client.post("/api/v1/pairing/claim", headers=headers,
                     json={"code": "ABC123", "passphrase": "pw"})
    assert r2.status_code == 403, r2.get_data(as_text=True)


def test_developer_clears_the_role_gate(client, app):
    """A developer passes the role gate; lookup then 404s (no such code),
    which proves the gate let them through rather than blocking with 403."""
    with app.app_context():
        headers = _headers(_user(User.ROLE_DEVELOPER))

    r = client.post("/api/v1/pairing/lookup", headers=headers, json={"code": "NOSUCH"})
    assert r.status_code != 403, r.get_data(as_text=True)
    assert r.status_code == 404, r.get_data(as_text=True)
