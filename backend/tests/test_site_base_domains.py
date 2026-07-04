"""Proving tests for the base-domain registry (multiple base domains).

A managed site can be published under any registered base domain — a.example.com
or a.toto.com — each with its own DNS mode / HTTPS state, defaulting to the one
marked default. Back-compat: the legacy single ``sites_base_domain`` setting is
materialised into the default row the first time a second domain is registered.
"""


def _set(key, value):
    from app import db
    from app.services.settings_service import SettingsService
    SettingsService.set(key, value)
    db.session.commit()


def _mk_app(name='Acme', port=8500, app_type='docker'):
    from app import db
    from app.models import User, Application
    from werkzeug.security import generate_password_hash
    u = User(email=f'{name}@bd.local', username=f'bd-{name}'.replace(' ', '-').lower(),
             password_hash=generate_password_hash('x'), role=User.ROLE_ADMIN, is_active=True)
    db.session.add(u)
    db.session.commit()
    a = Application(name=name, app_type=app_type, user_id=u.id, root_path=f'/srv/{name}', port=port)
    db.session.add(a)
    db.session.commit()
    return a


# ── seeding + registry CRUD ──────────────────────────────────────────────────

def test_add_seeds_legacy_default_then_registers_new(app, monkeypatch):
    from app.services.site_base_domain_service import SiteBaseDomainService
    from app.services.site_domain_service import SiteDomainService
    _set('sites_base_domain', 'example.com')
    _set('sites_https_enabled', True)

    res = SiteBaseDomainService.add('toto.com')
    assert res['success']
    domains = [r.domain for r in SiteBaseDomainService.list_rows()]
    assert domains == ['example.com', 'toto.com']            # default sorts first
    # The legacy domain became the default row, carrying its HTTPS state.
    assert SiteDomainService.base_domain() == 'example.com'
    assert SiteBaseDomainService.default().domain == 'example.com'
    assert SiteBaseDomainService.get('example.com').https_enabled is True
    assert SiteBaseDomainService.get('toto.com').is_default is False


def test_add_make_default_demotes_others(app, monkeypatch):
    from app.services.site_base_domain_service import SiteBaseDomainService
    _set('sites_base_domain', 'example.com')
    SiteBaseDomainService.add('toto.com', make_default=True)
    assert SiteBaseDomainService.default().domain == 'toto.com'
    assert SiteBaseDomainService.get('example.com').is_default is False


def test_remove_default_promotes_successor(app, monkeypatch):
    from app.services.site_base_domain_service import SiteBaseDomainService
    from app.services.site_domain_service import SiteDomainService
    _set('sites_base_domain', 'example.com')
    SiteBaseDomainService.add('toto.com')
    SiteBaseDomainService.remove('example.com')
    assert SiteDomainService.base_domain() == 'toto.com'
    assert SiteBaseDomainService.default().domain == 'toto.com'


def test_invalid_domain_rejected(app, monkeypatch):
    from app.services.site_base_domain_service import SiteBaseDomainService
    assert SiteBaseDomainService.add('not a domain').get('success') is False
    assert SiteBaseDomainService.add('nodots').get('success') is False


# ── base-aware resolution in SiteDomainService ───────────────────────────────

def test_subdomain_for_picks_the_named_base(app, monkeypatch):
    from app.services.site_base_domain_service import SiteBaseDomainService
    from app.services.site_domain_service import SiteDomainService
    _set('sites_base_domain', 'example.com')
    SiteBaseDomainService.add('toto.com')
    assert SiteDomainService.subdomain_for('My Blog') == 'my-blog.example.com'
    assert SiteDomainService.subdomain_for('My Blog', base='toto.com') == 'my-blog.toto.com'
    # An unregistered base falls back to the default, never publishes under it.
    assert SiteDomainService.subdomain_for('X', base='unmanaged.net') == 'x.example.com'


def test_covering_base_longest_match(app, monkeypatch):
    from app.services.site_base_domain_service import SiteBaseDomainService
    from app.services.site_domain_service import SiteDomainService
    _set('sites_base_domain', 'example.com')
    SiteBaseDomainService.add('apps.example.com')            # more specific base
    SiteBaseDomainService.add('toto.com')
    assert SiteDomainService.covering_base('a.toto.com') == 'toto.com'
    assert SiteDomainService.covering_base('a.apps.example.com') == 'apps.example.com'
    assert SiteDomainService.covering_base('a.example.com') == 'example.com'
    assert SiteDomainService.covers('a.nope.net') is False


def test_https_and_dns_mode_are_per_base(app, monkeypatch):
    from app.services.site_base_domain_service import SiteBaseDomainService
    from app.services.site_domain_service import SiteDomainService
    _set('sites_base_domain', 'example.com')
    _set('sites_https_enabled', True)                        # seeds example.com https=on
    SiteBaseDomainService.add('toto.com', dns_mode='per-site')
    assert SiteDomainService.https_enabled('example.com') is True
    assert SiteDomainService.https_enabled('toto.com') is False
    assert SiteDomainService.dns_mode('example.com') == 'wildcard'
    assert SiteDomainService.dns_mode('toto.com') == 'per-site'
    fc, key = SiteDomainService.wildcard_cert_paths('toto.com')
    assert fc.endswith('/toto.com/fullchain.pem')


# ── API: registry list + admin management ────────────────────────────────────

def test_base_domains_list_endpoint(app, client, auth_headers):
    from app.services.site_base_domain_service import SiteBaseDomainService
    _set('sites_base_domain', 'example.com')
    SiteBaseDomainService.add('toto.com')
    resp = client.get('/api/v1/domains/base-domains', headers=auth_headers)
    assert resp.status_code == 200
    data = resp.get_json()
    assert data['default'] == 'example.com'
    assert {b['domain'] for b in data['base_domains']} == {'example.com', 'toto.com'}


def test_base_domains_list_legacy_single(app, client, auth_headers):
    # No registry rows: falls back to the single configured base.
    _set('sites_base_domain', 'solo.example.com')
    resp = client.get('/api/v1/domains/base-domains', headers=auth_headers)
    data = resp.get_json()
    assert data['default'] == 'solo.example.com'
    assert data['base_domains'][0]['is_default'] is True


def test_admin_add_and_set_default_and_remove(app, client, auth_headers):
    _set('sites_base_domain', 'example.com')
    add = client.post('/api/v1/admin/sites-https/base-domains',
                      json={'domain': 'toto.com'}, headers=auth_headers)
    assert add.status_code == 201
    mk = client.post('/api/v1/admin/sites-https/base-domains/toto.com/default', headers=auth_headers)
    assert mk.status_code == 200 and mk.get_json()['base']['is_default'] is True
    rm = client.delete('/api/v1/admin/sites-https/base-domains/example.com', headers=auth_headers)
    assert rm.status_code == 200


def test_admin_update_dns_mode(app, client, auth_headers):
    from app.services.site_base_domain_service import SiteBaseDomainService
    _set('sites_base_domain', 'example.com')
    SiteBaseDomainService.add('toto.com')
    resp = client.patch('/api/v1/admin/sites-https/base-domains/toto.com',
                        json={'dns_mode': 'per-site'}, headers=auth_headers)
    assert resp.status_code == 200
    assert SiteBaseDomainService.get('toto.com').dns_mode == 'per-site'


def test_suggest_subdomain_honours_base(app, client, auth_headers):
    from app.services.site_base_domain_service import SiteBaseDomainService
    _set('sites_base_domain', 'example.com')
    SiteBaseDomainService.add('toto.com')
    a = _mk_app(name='My Service', port=8900)
    resp = client.get(f'/api/v1/domains/suggest-subdomain?application_id={a.id}&base=toto.com',
                      headers=auth_headers)
    assert resp.status_code == 200
    assert resp.get_json()['suggestion'] == 'my-service.toto.com'


def test_per_base_https_setup_persists_to_row(app, monkeypatch):
    from app import db
    from app.models.email import DNSProviderConfig
    from app.services.dns_provider_service import DNSProviderService
    from app.services.advanced_ssl_service import AdvancedSSLService
    from app.services.sites_https_service import SitesHttpsService
    from app.services.site_base_domain_service import SiteBaseDomainService
    from app.services.site_domain_service import SiteDomainService

    _set('sites_base_domain', 'example.com')
    SiteBaseDomainService.add('toto.com')                    # a second base, https off
    provider = DNSProviderConfig(name='cf', provider='cloudflare', api_key='tok', api_secret='sec')
    db.session.add(provider)
    _set('server_public_ip', '203.0.113.9')

    monkeypatch.setattr(DNSProviderService, 'ensure_a_record',
                        classmethod(lambda cls, host, ip: {'created': True, 'record': {}}))
    monkeypatch.setattr(AdvancedSSLService, 'issue_wildcard_cert', staticmethod(
        lambda domain, prov, creds, email=None: {'success': True,
                                                 'certificate_path': f'/etc/letsencrypt/live/{domain}/fullchain.pem'}))

    res = SitesHttpsService.setup(provider.id, base='toto.com')
    assert res['success'] and res['base_domain'] == 'toto.com'
    # Persisted to toto.com's row only — example.com stays HTTP.
    assert SiteBaseDomainService.get('toto.com').https_enabled is True
    assert SiteBaseDomainService.get('toto.com').dns_provider_config_id == provider.id
    assert SiteDomainService.https_enabled('toto.com') is True
    assert SiteDomainService.https_enabled('example.com') is False


def test_give_subdomain_under_chosen_base(app, monkeypatch):
    from app.services.site_base_domain_service import SiteBaseDomainService
    from app.services.site_domain_service import SiteDomainService
    from app.services.nginx_service import NginxService
    from app.models.domain import Domain
    _set('sites_base_domain', 'example.com')
    SiteBaseDomainService.add('toto.com')
    a = _mk_app(name='Shop', port=8600)

    monkeypatch.setattr(NginxService, 'create_site', staticmethod(lambda **k: {'success': True}))
    monkeypatch.setattr(NginxService, 'enable_site', staticmethod(lambda name: {'success': True}))

    res = SiteDomainService.give_subdomain(a, base='toto.com')
    assert res['success'] and res['host'] == 'shop.toto.com'
    assert Domain.query.filter_by(name='shop.toto.com', application_id=a.id).first() is not None
