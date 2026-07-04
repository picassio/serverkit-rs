"""Cross-cutting smoke/integration test for the recently shipped capabilities.

This is deliberately broad and shallow: the goal is to prove the new endpoints
are *wired and responding sensibly* (registered on the URL map, auth enforced
where expected, list endpoints return their documented envelope), NOT to
re-test deep behavior — each capability already has a focused suite
(``test_buildpacks``, ``test_proxy_stack``, ``test_projects``, ``test_previews``,
``test_container_status``, ``test_api_scopes``, ``test_config_snapshots``,
``test_shared_resources``, ``test_server_onboarding``, ``test_template_*``).

Capabilities exercised here (matches docs/ENHANCEMENTS.md):

  1. Container status aggregator   /api/v1/status/app/<id>, /api/v1/status/apps
  2. API token scopes              /api/v1/api-keys/scopes
  3. Server onboarding state mc.   /api/v1/servers/<id>/onboarding/*
  4. Declarative template catalog  /api/v1/templates/catalog/schema
  5. Build packs                   /api/v1/buildpacks/detect|generate
  6. Deployment config snapshots   /api/v1/apps/<id>/snapshots[...]
  7. Projects / Environments       /api/v1/projects, /api/v1/environments
  8. Polymorphic shared resources  /api/v1/shared/...
  9. PR preview environments       /api/v1/apps/<id>/previews, webhook
 10. Per-server proxy stack        /api/v1/servers/<id>/proxy/compose-preview

Models imported at module top so they are registered on ``db.metadata`` before
the ``app`` fixture's ``create_all()`` runs (several services import them lazily).
"""
import pytest
import yaml

# Force-register the new tables on the shared metadata before create_all().
from app.models.proxy_stack import ProxyStack  # noqa: F401
from app.models.deployment_snapshot import DeploymentSnapshot  # noqa: F401
from app.models.application_preview import ApplicationPreview  # noqa: F401
from app.models.project import Project  # noqa: F401
from app.models.environment import Environment  # noqa: F401
from app.models.shared_resource import (  # noqa: F401
    ResourceTag,
    SharedVariableGroup,
)


# --------------------------------------------------------------------------- #
# Module-scoped app/client/auth fixtures.
#
# conftest's `app` fixture is function-scoped: every test spins up a fresh
# create_app() (which starts background workers) and drop_all/create_all's the
# *shared file-backed* test DB. Running this whole cross-cutting module that way
# races those background workers against teardown's drop_all ("no such table"
# during a later setup). Overriding the fixtures at MODULE scope means ONE app
# and ONE schema for the entire module — deterministic, and still a real
# end-to-end exercise of the live routes. These overrides shadow conftest's for
# this module only.
# --------------------------------------------------------------------------- #
@pytest.fixture(scope='module')
def app():
    from app import create_app
    from app import db as _db

    application = create_app('testing')
    with application.app_context():
        _db.create_all()
        yield application
        _db.session.remove()
        _db.drop_all()


@pytest.fixture(scope='module')
def client(app):
    return app.test_client()


@pytest.fixture(scope='module')
def auth_headers(app):
    """An admin JWT, created once for the module (idempotent)."""
    from app import db
    from app.models import User
    from flask_jwt_extended import create_access_token
    from werkzeug.security import generate_password_hash

    with app.app_context():
        user = User.query.filter_by(username='enh-testadmin').first()
        if user is None:
            user = User(
                email='enh-testadmin@test.local', username='enh-testadmin',
                password_hash=generate_password_hash('testpass'),
                role=User.ROLE_ADMIN, is_active=True,
            )
            db.session.add(user)
            db.session.commit()
        token = create_access_token(identity=user.id)
    return {'Authorization': f'Bearer {token}'}


# --------------------------------------------------------------------------- #
# helpers
# --------------------------------------------------------------------------- #
_counter = {'n': 0}


def _uniq(prefix):
    _counter['n'] += 1
    return f'{prefix}-{_counter["n"]}'


def _has_rule(app, rule_substr):
    """True if any registered URL rule contains ``rule_substr``."""
    return any(rule_substr in str(r) for r in app.url_map.iter_rules())


def _make_server(app):
    """Create one Server row, return its (string UUID) id."""
    from app import db
    from app.models.server import Server
    s = Server(name=_uniq('enh-host'), agent_id=_uniq('agent-enh'))
    db.session.add(s)
    db.session.commit()
    return s.id


def _make_app(app, name='enh-app'):
    """Create a minimal Application row (owned by a throwaway user), return id."""
    from app import db
    from app.models.application import Application
    from app.models.user import User
    from werkzeug.security import generate_password_hash

    owner = User.query.filter_by(username='enh-owner').first()
    if owner is None:
        owner = User(
            email='enh-owner@test.local', username='enh-owner',
            password_hash=generate_password_hash('x'),
            role=User.ROLE_ADMIN, is_active=True,
        )
        db.session.add(owner)
        db.session.commit()
    a = Application(name=_uniq(name), app_type='docker', user_id=owner.id)
    db.session.add(a)
    db.session.commit()
    return a.id


# --------------------------------------------------------------------------- #
# 0. auth is enforced
# --------------------------------------------------------------------------- #
# Consolidated into one test (one app instance): the file-backed test DB is
# shared across the suite and each create_app() spins up background workers, so
# minimizing app churn keeps the cross-cutting module deterministic.
def test_new_protected_endpoints_require_auth(client):
    """Every new JWT-protected endpoint rejects an anonymous request."""
    read_paths = [
        '/api/v1/status/apps',
        '/api/v1/api-keys/scopes',
        '/api/v1/templates/catalog/schema',
        '/api/v1/projects',
        '/api/v1/shared/resource-types',
    ]
    for path in read_paths:
        resp = client.get(path)
        assert resp.status_code in (401, 422), (path, resp.status_code)

    # A protected write endpoint behaves the same.
    resp = client.post('/api/v1/buildpacks/generate', json={'plan': {}})
    assert resp.status_code in (401, 422)


# --------------------------------------------------------------------------- #
# 1. Container status aggregator
# --------------------------------------------------------------------------- #
def test_container_status_apps_returns_list(client, auth_headers):
    resp = client.get('/api/v1/status/apps', headers=auth_headers)
    assert resp.status_code == 200
    body = resp.get_json()
    assert isinstance(body.get('statuses'), list)


def test_container_status_single_app_responds(client, auth_headers, app):
    with app.app_context():
        app_id = _make_app(app, 'status-app')
    resp = client.get(f'/api/v1/status/app/{app_id}', headers=auth_headers)
    assert resp.status_code == 200
    body = resp.get_json()
    # Aggregator always yields a status string (never raises), even with no
    # Docker available in the test environment.
    assert 'status' in body


# --------------------------------------------------------------------------- #
# 2. API token scopes catalog
# --------------------------------------------------------------------------- #
def test_api_key_scopes_catalog_non_empty(client, auth_headers):
    resp = client.get('/api/v1/api-keys/scopes', headers=auth_headers)
    assert resp.status_code == 200
    scopes = resp.get_json().get('scopes')
    assert isinstance(scopes, list) and len(scopes) > 0
    # Each row is the documented {key,label,group,description} shape.
    assert all('key' in s for s in scopes)
    keys = {s['key'] for s in scopes}
    assert 'read' in keys and 'write' in keys


# --------------------------------------------------------------------------- #
# 3. Server onboarding state machine
# --------------------------------------------------------------------------- #
def test_onboarding_status_responds_for_server(client, auth_headers, app):
    with app.app_context():
        server_id = _make_server(app)
    resp = client.get(
        f'/api/v1/servers/{server_id}/onboarding/status', headers=auth_headers)
    # 200 with a state machine payload (or a graceful service error), never 404.
    assert resp.status_code == 200, resp.get_json()


def test_onboarding_status_unknown_server_404(client, auth_headers):
    resp = client.get(
        '/api/v1/servers/does-not-exist/onboarding/status', headers=auth_headers)
    assert resp.status_code == 404


def test_onboarding_routes_registered(app):
    for sub in ('/onboarding/start', '/onboarding/retry', '/onboarding/status'):
        assert _has_rule(app, sub), sub


# --------------------------------------------------------------------------- #
# 4. Declarative template catalog schema
# --------------------------------------------------------------------------- #
def test_template_catalog_schema_exposes_magic_variables(client, auth_headers):
    resp = client.get('/api/v1/templates/catalog/schema', headers=auth_headers)
    assert resp.status_code == 200
    body = resp.get_json()
    assert 'variable_types' in body and body['variable_types']
    tokens = {mv['token'] for mv in body.get('magic_variables', [])}
    # The documented SERVICE_* magic-variable family.
    assert '${SERVICE_PASSWORD_<NAME>}' in tokens
    assert '${SERVICE_USER_<NAME>}' in tokens
    assert '${SERVICE_FQDN_<NAME>}' in tokens
    assert '${SERVICE_URL_<NAME>}' in tokens
    assert '${SERVICE_BASE64_<NAME>}' in tokens


# --------------------------------------------------------------------------- #
# 5. Build packs — generate is pure, returns a Dockerfile for a simple plan
# --------------------------------------------------------------------------- #
def test_buildpacks_generate_returns_dockerfile(client, auth_headers):
    plan = {
        'builder': 'nixpacks', 'language': 'node', 'framework': 'express',
        'versions': {'node': '20'}, 'build_command': 'npm run build',
        'start_command': 'node server.js', 'port': 3000,
    }
    resp = client.post(
        '/api/v1/buildpacks/generate',
        json={'plan': plan, 'name': 'demo'},
        headers=auth_headers,
    )
    assert resp.status_code == 200
    body = resp.get_json()
    assert 'FROM node:20' in body['dockerfile']
    assert 'EXPOSE 3000' in body['dockerfile']
    assert 'services:' in body['compose']


def test_buildpacks_generate_rejects_missing_plan(client, auth_headers):
    resp = client.post(
        '/api/v1/buildpacks/generate', json={}, headers=auth_headers)
    assert resp.status_code == 400


# --------------------------------------------------------------------------- #
# 6. Deployment config snapshots
# --------------------------------------------------------------------------- #
def test_snapshots_list_returns_envelope(client, auth_headers, app):
    with app.app_context():
        app_id = _make_app(app, 'snap-app')
    resp = client.get(
        f'/api/v1/apps/{app_id}/snapshots', headers=auth_headers)
    assert resp.status_code == 200
    assert isinstance(resp.get_json().get('snapshots'), list)


def test_snapshots_unknown_app_404(client, auth_headers):
    resp = client.get('/api/v1/apps/999999/snapshots', headers=auth_headers)
    assert resp.status_code == 404


# --------------------------------------------------------------------------- #
# 7. Projects / Environments round-trip
# --------------------------------------------------------------------------- #
def test_projects_create_and_list_roundtrip(client, auth_headers):
    # List starts as a well-formed envelope.
    resp = client.get('/api/v1/projects', headers=auth_headers)
    assert resp.status_code == 200
    assert isinstance(resp.get_json().get('projects'), list)

    # Create a project (auto-creates a default environment).
    created = client.post(
        '/api/v1/projects',
        json={'name': 'Integration Project'},
        headers=auth_headers,
    )
    assert created.status_code == 201, created.get_json()
    project = created.get_json()['project']
    assert project['name'] == 'Integration Project'
    assert project.get('environments'), 'a default environment should exist'

    # It now shows up in the list.
    listed = client.get('/api/v1/projects', headers=auth_headers).get_json()
    assert any(p['id'] == project['id'] for p in listed['projects'])


def test_environment_create_under_project(client, auth_headers):
    created = client.post(
        '/api/v1/projects', json={'name': 'Env Host Project'},
        headers=auth_headers,
    )
    project_id = created.get_json()['project']['id']
    env = client.post(
        '/api/v1/environments',
        json={'project_id': project_id, 'name': 'staging'},
        headers=auth_headers,
    )
    assert env.status_code == 201, env.get_json()
    assert env.get_json()['environment']['name'] == 'staging'


# --------------------------------------------------------------------------- #
# 8. Polymorphic shared resources
# --------------------------------------------------------------------------- #
def test_shared_resource_types_listed(client, auth_headers):
    resp = client.get('/api/v1/shared/resource-types', headers=auth_headers)
    assert resp.status_code == 200
    assert isinstance(resp.get_json().get('resource_types'), list)


def test_shared_variable_groups_list_envelope(client, auth_headers):
    resp = client.get('/api/v1/shared/variable-groups', headers=auth_headers)
    assert resp.status_code == 200
    assert isinstance(resp.get_json().get('groups'), list)


# --------------------------------------------------------------------------- #
# 9. PR preview environments
# --------------------------------------------------------------------------- #
def test_previews_list_returns_envelope(client, auth_headers, app):
    with app.app_context():
        app_id = _make_app(app, 'preview-app')
    resp = client.get(
        f'/api/v1/apps/{app_id}/previews', headers=auth_headers)
    assert resp.status_code == 200
    assert isinstance(resp.get_json().get('previews'), list)


def test_pull_request_webhook_registered_and_handles_unknown_source(app, client):
    # Route is mounted (public — no JWT).
    assert _has_rule(app, '/webhooks/pull-request/')
    # An unsigned, source-less delivery is rejected cleanly (not a 500).
    resp = client.post('/api/v1/webhooks/pull-request/sometoken', json={})
    assert resp.status_code in (400, 404)


# --------------------------------------------------------------------------- #
# 10. Per-server managed proxy stack
# --------------------------------------------------------------------------- #
def test_proxy_compose_preview_returns_yaml_for_traefik(client, auth_headers, app):
    with app.app_context():
        server_id = _make_server(app)
    resp = client.get(
        f'/api/v1/servers/{server_id}/proxy/compose-preview?proxy_type=traefik',
        headers=auth_headers,
    )
    assert resp.status_code == 200
    body = resp.get_json()
    assert body['proxy_type'] == 'traefik'
    # The compose is real, parseable YAML describing a proxy service.
    doc = yaml.safe_load(body['compose'])
    assert 'proxy' in doc['services']


def test_proxy_compose_preview_nginx_is_null(client, auth_headers, app):
    with app.app_context():
        server_id = _make_server(app)
    resp = client.get(
        f'/api/v1/servers/{server_id}/proxy/compose-preview?proxy_type=nginx',
        headers=auth_headers,
    )
    assert resp.status_code == 200
    # Host nginx is the default — no managed compose to render.
    assert resp.get_json()['compose'] is None


# --------------------------------------------------------------------------- #
# cross-cutting: every new blueprint is actually mounted
# --------------------------------------------------------------------------- #
def test_enhancement_routes_registered(app):
    """All ten capabilities expose their advertised routes on the URL map."""
    expected = [
        '/api/v1/status/apps',
        '/api/v1/status/app/',
        '/api/v1/api-keys/scopes',
        '/api/v1/templates/catalog/schema',
        '/api/v1/buildpacks/detect',
        '/api/v1/buildpacks/generate',
        '/api/v1/apps/<int:app_id>/snapshots',
        '/api/v1/apps/<int:app_id>/snapshots/<int:snap_id>/diff',
        '/api/v1/apps/<int:app_id>/snapshots/<int:snap_id>/restore',
        '/api/v1/projects',
        '/api/v1/environments',
        '/api/v1/shared/resource-types',
        '/api/v1/shared/variable-groups',
        '/api/v1/apps/<int:app_id>/previews',
        '/api/v1/webhooks/pull-request/',
        '/proxy/compose-preview',
        '/proxy/configure',
        '/proxy/switch',
    ]
    rules = {str(r) for r in app.url_map.iter_rules()}
    missing = [e for e in expected if not any(e in r for r in rules)]
    assert not missing, f'missing routes: {missing}'
