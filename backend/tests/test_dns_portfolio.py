"""Proving tests for the DNS domain portfolio (Domains-page "see all my domains").

Before this, a connected Cloudflare account's zones never surfaced anywhere — the
Domains page only listed app-linked rows and DNS Zones only listed hand-added
zones. ``DNSZoneService.list_portfolio`` reads each connected provider's account
live and merges it with the locally-adopted zones, and ``adopt_zone`` lazily
materializes a managed zone row on demand (idempotently). These tests prove:

* provider zones surface as un-adopted portfolio rows,
* an already-adopted domain is flagged with its local zone id + record count,
* a token that can't enumerate zones is reported under ``errors`` (not swallowed),
* adopt is idempotent — clicking "Manage" twice never duplicates a zone, and
* the HTTP endpoint returns the portfolio to an authenticated caller.
"""

from app.services.dns_provider_service import DNSProviderService
from app.services.dns_zone_service import DNSZoneService


def _add_connection(name='cf', token='tok'):
    from app.models.email import DNSProviderConfig
    DNSProviderService.add_provider(name=name, provider='cloudflare', api_key=token)
    return DNSProviderConfig.query.filter_by(name=name).first()


def _stub_zones(monkeypatch, result):
    """Make every DNSProviderService.list_zones(config_id) return ``result``."""
    monkeypatch.setattr(DNSProviderService, 'list_zones',
                        classmethod(lambda cls, cid: result))


def test_portfolio_lists_provider_zones_unadopted(app, monkeypatch):
    conn = _add_connection()
    _stub_zones(monkeypatch, {'success': True, 'zones': [
        {'id': 'z1', 'name': 'example.com', 'status': 'active'},
        {'id': 'z2', 'name': 'shop.example.com', 'status': 'pending'},
    ]})

    out = DNSZoneService.list_portfolio()
    by_domain = {d['domain']: d for d in out['domains']}

    assert set(by_domain) == {'example.com', 'shop.example.com'}
    apex = by_domain['example.com']
    assert apex['provider'] == 'cloudflare'
    assert apex['provider_zone_id'] == 'z1'
    assert apex['config_id'] == conn.id
    assert apex['adopted'] is False and apex['zone_id'] is None
    assert out['errors'] == []
    assert any(p['id'] == conn.id for p in out['providers'])


def test_portfolio_marks_adopted_zone(app, monkeypatch):
    from app import db
    from app.models.dns_zone import DNSZone

    conn = _add_connection()
    _stub_zones(monkeypatch, {'success': True, 'zones': [
        {'id': 'z1', 'name': 'example.com', 'status': 'active'},
    ]})

    zone = DNSZone(domain='example.com', provider='cloudflare',
                   provider_zone_id='z1', dns_provider_config_id=conn.id)
    db.session.add(zone)
    db.session.commit()

    apex = next(d for d in DNSZoneService.list_portfolio()['domains']
                if d['domain'] == 'example.com')
    assert apex['adopted'] is True
    assert apex['zone_id'] == zone.id
    assert apex['record_count'] == 0


def test_portfolio_surfaces_scoped_token_error(app, monkeypatch):
    """A single-zone scoped token can't list zones — surface it, don't show nothing."""
    conn = _add_connection()
    _stub_zones(monkeypatch, {'success': False, 'error': 'token lacks Zone:Read'})

    out = DNSZoneService.list_portfolio()
    assert out['domains'] == []
    assert len(out['errors']) == 1
    assert out['errors'][0]['config_id'] == conn.id
    assert 'Zone:Read' in out['errors'][0]['error']


def test_adopt_zone_is_idempotent(app, monkeypatch):
    from app.models.dns_zone import DNSZone

    conn = _add_connection()
    # create_zone matches the typed domain to a provider zone via list_zones.
    _stub_zones(monkeypatch, {'success': True, 'zones': [
        {'id': 'z1', 'name': 'example.com'},
    ]})

    z1 = DNSZoneService.adopt_zone('example.com', conn.id)
    assert z1.provider == 'cloudflare'
    assert z1.provider_zone_id == 'z1'
    assert z1.dns_provider_config_id == conn.id

    z2 = DNSZoneService.adopt_zone('EXAMPLE.com', conn.id)  # case-insensitive, again
    assert z2.id == z1.id
    assert DNSZone.query.filter_by(domain='example.com').count() == 1


def test_portfolio_enriches_cloudflare_registrar_expiry(app, monkeypatch):
    """A Cloudflare-registered domain gets its expiry + auto-renew from the Registrar
    API; a zone registered elsewhere (no registrar entry) shows none."""
    from app.services.dns import CloudflareClient

    _add_connection()
    _stub_zones(monkeypatch, {'success': True, 'zones': [
        {'id': 'z1', 'name': 'example.com', 'status': 'active', 'account_id': 'acct1'},
        {'id': 'z2', 'name': 'elsewhere.com', 'status': 'active', 'account_id': 'acct1'},
    ]})
    monkeypatch.setattr(CloudflareClient, 'list_registrar_domains',
                        lambda self, acct: {'success': True, 'domains': [
                            {'name': 'example.com', 'expires_at': '2027-01-01T00:00:00Z',
                             'auto_renew': True, 'registrar': 'Cloudflare'},
                        ]})

    by_domain = {d['domain']: d for d in DNSZoneService.list_portfolio()['domains']}
    assert by_domain['example.com']['expires_at'] == '2027-01-01T00:00:00Z'
    assert by_domain['example.com']['auto_renew'] is True
    # A zone not registered at Cloudflare has no registrar row → no expiry.
    assert by_domain['elsewhere.com']['expires_at'] is None
    assert by_domain['elsewhere.com']['auto_renew'] is None


def test_provider_records_by_ref_lists_live_records(app, monkeypatch):
    """The drawer reads a domain's live records by connection + provider zone id,
    without adopting first; each record is tagged owned vs external."""
    from app.services.dns import CloudflareClient

    conn = _add_connection()
    monkeypatch.setattr(CloudflareClient, 'list_records',
                        lambda self, zid: {'success': True, 'records': [
                            {'id': 'r1', 'type': 'A', 'name': 'example.com',
                             'content': '1.2.3.4', 'ttl': 1, 'proxied': True, 'priority': None},
                        ]})

    res = DNSZoneService.list_provider_records_by_ref(conn.id, 'zoneX')
    assert res['success'] is True
    assert res['records'][0]['type'] == 'A'
    assert res['records'][0]['proxied'] is True
    # Nothing in the ownership ledger → classified as the user's own record.
    assert res['records'][0]['managed_by'] == 'external'
    assert res['counts'] == {'serverkit': 0, 'external': 1}


def test_provider_records_by_ref_requires_cloudflare(app):
    res = DNSZoneService.list_provider_records_by_ref(999999, 'zoneX')
    assert res['success'] is False


def test_portfolio_endpoint_returns_domains(app, client, auth_headers, monkeypatch):
    _add_connection()
    _stub_zones(monkeypatch, {'success': True, 'zones': [
        {'id': 'z1', 'name': 'example.com', 'status': 'active'},
    ]})

    resp = client.get('/api/v1/dns/portfolio', headers=auth_headers)
    assert resp.status_code == 200
    body = resp.get_json()
    assert [d['domain'] for d in body['domains']] == ['example.com']
    assert body['domains'][0]['adopted'] is False


def test_lookup_domain_registration_rdap(app, monkeypatch):
    """RDAP fallback parses the expiration event + registrar name from the response."""
    import requests

    class _Resp:
        status_code = 200

        def json(self):
            return {
                'events': [
                    {'eventAction': 'registration', 'eventDate': '2020-01-01T00:00:00Z'},
                    {'eventAction': 'expiration', 'eventDate': '2027-03-04T00:00:00Z'},
                ],
                'entities': [{
                    'roles': ['registrar'],
                    'vcardArray': ['vcard', [['version', {}, 'text', '4.0'],
                                             ['fn', {}, 'text', 'MarkMonitor Inc.']]],
                }],
            }

    monkeypatch.setattr(requests, 'get', lambda url, headers=None, timeout=None: _Resp())
    res = DNSZoneService.lookup_domain_registration('example.com')
    assert res['success'] is True
    # Round-trips through the cache as a parsed datetime (Zulu normalized away).
    assert res['expires_at'].startswith('2027-03-04')
    assert res['registrar'] == 'MarkMonitor Inc.'


def test_lookup_domain_registration_not_found(app, monkeypatch):
    import requests

    class _Resp:
        status_code = 404

        def json(self):
            return {}

    monkeypatch.setattr(requests, 'get', lambda url, headers=None, timeout=None: _Resp())
    assert DNSZoneService.lookup_domain_registration('nope.invalid')['success'] is False


def test_registration_persists_and_caches(app, monkeypatch):
    """A lookup is saved to the cache table and a second call is served from it
    without re-querying RDAP — so the data survives a page refresh."""
    from app.models.domain_registration import DomainRegistration

    calls = {'n': 0}

    def fake_rdap(domain):
        calls['n'] += 1
        return {'success': True, 'expires_at': '2027-03-04T00:00:00Z', 'registrar': 'GoDaddy.com, LLC'}

    monkeypatch.setattr(DNSZoneService, '_rdap_query', staticmethod(fake_rdap))

    r1 = DNSZoneService.lookup_domain_registration('builditdesign.com')
    assert r1['success'] and r1['registrar'] == 'GoDaddy.com, LLC'
    row = DomainRegistration.query.filter_by(domain='builditdesign.com').first()
    assert row is not None and row.expires_at is not None

    r2 = DNSZoneService.lookup_domain_registration('builditdesign.com')
    assert r2.get('cached') is True
    assert calls['n'] == 1  # second call hit the cache, not the network


def test_portfolio_merges_cached_registration(app, monkeypatch):
    """Expiry looked up earlier (and cached) shows up in the portfolio/table even
    though the provider supplies none."""
    from app import db
    from datetime import datetime
    from app.models.domain_registration import DomainRegistration

    _add_connection()
    _stub_zones(monkeypatch, {'success': True, 'zones': [
        {'id': 'z1', 'name': 'builditdesign.com', 'status': 'active'},  # no account_id → no registrar call
    ]})
    db.session.add(DomainRegistration(
        domain='builditdesign.com', expires_at=datetime(2027, 3, 4),
        registrar='GoDaddy.com, LLC', source='rdap'))
    db.session.commit()

    apex = next(d for d in DNSZoneService.list_portfolio()['domains']
                if d['domain'] == 'builditdesign.com')
    assert apex['expires_at'] is not None
    assert apex['registrar'] == 'GoDaddy.com, LLC'
