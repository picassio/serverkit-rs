"""ProxyStackService tests.

The pure generators (``generate_compose`` / ``generate_config`` / hashing) are
exercised with no app context or Docker — they assert the *rendered* compose &
config, the same style as test_nginx_remote_upstream / test_fail2ban_jail.
The row-management tests use the ``app`` fixture for a clean DB and import the
ProxyStack model directly.
"""
import yaml
import pytest

from app.services.proxy_stack_service import ProxyStackService as PS
# Import the model at module top so it's registered on db.metadata before the
# `app` fixture runs db.create_all() (the service only imports it lazily).
from app.models.proxy_stack import ProxyStack  # noqa: F401


# ---------------------------------------------------------------------------
# Pure: generate_compose
# ---------------------------------------------------------------------------

def test_compose_nginx_is_none():
    assert PS.generate_compose('srv-1234abcd', 'nginx', {}) is None


def test_compose_traefik_is_valid_yaml_with_expected_bits():
    raw = PS.generate_compose('srv-1234abcd', 'traefik', {})
    assert raw is not None
    doc = yaml.safe_load(raw)  # parseable

    svc = doc['services']['proxy']
    assert svc['image'] == PS.TRAEFIK_IMAGE
    assert 'traefik' in svc['image']
    # Ports 80/443 published
    assert '80:80' in svc['ports']
    assert '443:443' in svc['ports']
    # External serverkit network present + flagged external
    assert PS.NETWORK_NAME in svc['networks']
    assert doc['networks'][PS.NETWORK_NAME]['external'] is True
    # Docker provider labels/command wired
    assert any('providers.docker' in c for c in svc['command'])


def test_compose_caddy_is_valid_yaml_with_expected_bits():
    raw = PS.generate_compose('srv-1234abcd', 'caddy', {})
    assert raw is not None
    doc = yaml.safe_load(raw)

    svc = doc['services']['proxy']
    assert svc['image'] == PS.CADDY_IMAGE
    assert 'caddy' in svc['image']
    assert '80:80' in svc['ports']
    assert '443:443' in svc['ports']
    assert PS.NETWORK_NAME in svc['networks']
    assert doc['networks'][PS.NETWORK_NAME]['external'] is True
    # Caddy persists certs/config in named volumes
    assert 'caddy_data' in doc['volumes']


def test_compose_traefik_acme_optional():
    raw = PS.generate_compose('srv-x', 'traefik', {'acme_email': 'ops@example.com'})
    doc = yaml.safe_load(raw)
    cmd = doc['services']['proxy']['command']
    assert any('acme.email=ops@example.com' in c for c in cmd)


def test_compose_caddy_acme_optional():
    raw = PS.generate_compose('srv-x', 'caddy', {'acme_email': 'ops@example.com'})
    doc = yaml.safe_load(raw)
    env = doc['services']['proxy']['environment']
    assert env['CADDY_EMAIL'] == 'ops@example.com'


def test_compose_unknown_type_raises():
    with pytest.raises(ValueError):
        PS.generate_compose('srv-x', 'haproxy', {})


# ---------------------------------------------------------------------------
# Pure: generate_config
# ---------------------------------------------------------------------------

SITES = [
    {'domain': 'a.example.com', 'upstream': 'app1:8001', 'tls': True},
    {'domain': 'b.example.com', 'upstream': 'app2:8002'},
]


def test_config_nginx_is_none():
    assert PS.generate_config('nginx', SITES) is None


def test_config_traefik_dynamic_routes_and_services():
    raw = PS.generate_config('traefik', SITES)
    doc = yaml.safe_load(raw)
    routers = doc['http']['routers']
    services = doc['http']['services']
    # One router + service per site
    assert len(routers) == 2
    assert len(services) == 2
    # Host rule + upstream url rendered
    rules = [r['rule'] for r in routers.values()]
    assert 'Host(`a.example.com`)' in rules
    urls = [s['loadBalancer']['servers'][0]['url'] for s in services.values()]
    assert 'http://app1:8001' in urls


def test_config_traefik_tls_uses_cert_resolver():
    raw = PS.generate_config('traefik', SITES)
    doc = yaml.safe_load(raw)
    tls_routers = [r for r in doc['http']['routers'].values() if 'tls' in r]
    assert tls_routers
    assert tls_routers[0]['tls']['certResolver'] == 'serverkit'


def test_config_caddyfile_blocks():
    raw = PS.generate_config('caddy', SITES)
    assert 'a.example.com {' in raw
    assert 'reverse_proxy app1:8001' in raw
    assert 'b.example.com {' in raw


def test_config_custom_snippet_appended():
    snippet = 'log { output stdout }'
    raw = PS.generate_config('caddy', SITES, custom_snippet=snippet)
    assert snippet in raw


# ---------------------------------------------------------------------------
# Pure: hashing / backup naming determinism
# ---------------------------------------------------------------------------

def test_config_hash_is_deterministic():
    content = "services:\n  proxy:\n    image: traefik\n"
    h1 = PS.config_hash(content)
    h2 = PS.config_hash(content)
    assert h1 == h2
    assert len(h1) == 8
    # Different content → different hash
    assert PS.config_hash(content + 'x') != h1


def test_backup_config_roundtrip(tmp_path, monkeypatch):
    """backup_config copies the live compose to a hash-named backup."""
    server_id = 'srv-backup01'
    stack_dir = tmp_path / 'proxy' / server_id
    stack_dir.mkdir(parents=True)
    compose_content = "services:\n  proxy:\n    image: caddy\n"
    (stack_dir / 'docker-compose.yml').write_text(compose_content)

    monkeypatch.setattr(PS, '_stack_dir', staticmethod(lambda sid: str(stack_dir)))

    backup_path = PS.backup_config(server_id)
    assert backup_path is not None
    # Backup filename carries the deterministic 8-char hash.
    digest = PS.config_hash(compose_content)
    assert digest in backup_path
    assert backup_path.endswith('.yml')
    with open(backup_path, encoding='utf-8') as f:
        assert f.read() == compose_content


def test_backup_config_missing_is_none(tmp_path, monkeypatch):
    monkeypatch.setattr(
        PS, '_stack_dir', staticmethod(lambda sid: str(tmp_path / 'nope'))
    )
    assert PS.backup_config('srv-none') is None


# ---------------------------------------------------------------------------
# DB-backed: get_or_create / configure / switch
# ---------------------------------------------------------------------------

def _make_server(app):
    from app import db
    from app.models.server import Server
    s = Server(name='proxy-host', agent_id='agent-proxy-1')
    db.session.add(s)
    db.session.commit()
    return s.id


def test_get_or_create_defaults_to_nginx(app):
    from app.models.proxy_stack import ProxyStack
    with app.app_context():
        sid = _make_server(app)
        stack = PS.get_or_create(sid)
        assert stack.proxy_type == 'nginx'
        assert stack.server_id == sid
        # Idempotent: no duplicate rows.
        again = PS.get_or_create(sid)
        assert again.id == stack.id
        assert ProxyStack.query.filter_by(server_id=sid).count() == 1


def test_configure_sets_snippet_and_path(app):
    with app.app_context():
        sid = _make_server(app)
        stack = PS.configure(sid, proxy_type='traefik', custom_snippet='# hi')
        assert stack.proxy_type == 'traefik'
        assert stack.custom_snippet == '# hi'
        assert stack.compose_path and stack.compose_path.endswith('docker-compose.yml')


def test_switch_flips_rows(app):
    with app.app_context():
        sid = _make_server(app)
        PS.get_or_create(sid)
        s1 = PS.switch(sid, 'caddy')
        assert s1.proxy_type == 'caddy'
        assert s1.compose_path is not None
        # Switching back to nginx clears the compose path.
        s2 = PS.switch(sid, 'nginx')
        assert s2.proxy_type == 'nginx'
        assert s2.compose_path is None


def test_configure_rejects_unknown_type(app):
    with app.app_context():
        sid = _make_server(app)
        with pytest.raises(ValueError):
            PS.configure(sid, proxy_type='haproxy')


def test_create_app_testing_still_works():
    """Sanity: the app factory + our model import cleanly."""
    from app import create_app
    from app.models.proxy_stack import ProxyStack  # noqa: F401
    app = create_app('testing')
    assert app is not None
