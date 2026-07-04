"""Proving tests for DNS ownership + the never-touch-foreign guard (Phase 1).

ServerKit records every provider DNS record it creates in managed_dns_records, and
gates writes so it never overwrites or deletes a record the *user* created (their
own "Maria & Pedro" records). Automatic paths refuse foreign records; the explicit
Zones page adopts them. The mirror classifies a zone's live records accordingly.
"""


class _Resp:
    def __init__(self, js):
        self._js = js

    def json(self):
        return self._js


class FakeClient:
    """Duck-typed CloudflareClient for guard tests (no HTTP)."""
    def __init__(self, existing_id=None, upsert_result=None):
        self.existing_id = existing_id
        self.upsert_result = upsert_result or {'success': True, 'record_id': 'NEW'}
        self.calls = []

    def find_record_id(self, zone_id, record_type, name, caa=None):
        self.calls.append(('find', record_type, name))
        return self.existing_id

    def upsert(self, zone_id, spec, record_id=None):
        self.calls.append(('upsert', spec.record_type, spec.name, record_id))
        return self.upsert_result

    def delete(self, zone_id, record_id=None, record_type=None, name=None):
        self.calls.append(('delete', record_id, record_type, name))
        return {'success': True}


def _verbs(client):
    return [c[0] for c in client.calls]


# ── ledger reads/writes ──────────────────────────────────────────────────────

def test_record_write_and_owns(app):
    from app.services.dns_ownership_service import DnsOwnershipService as O
    O.record_write('cloudflare', 'z1', 'A', 'www.example.com',
                   provider_record_id='R1', content='1.2.3.4', source='zone')
    assert O.owns('z1', provider_record_id='R1')
    assert O.owns('z1', record_type='A', name='www.example.com')
    assert not O.owns('z1', record_type='A', name='maria.example.com')
    assert not O.owns('other-zone', provider_record_id='R1')


def test_record_write_is_upsert(app):
    from app.services.dns_ownership_service import DnsOwnershipService as O
    from app.models.managed_dns_record import ManagedDnsRecord
    O.record_write('cloudflare', 'z1', 'A', 'www.example.com', provider_record_id='R1', source='zone')
    O.record_write('cloudflare', 'z1', 'A', 'www.example.com', provider_record_id='R1',
                   content='9.9.9.9', source='zone')
    rows = ManagedDnsRecord.query.filter_by(provider_zone_id='z1', name='www.example.com').all()
    assert len(rows) == 1 and rows[0].content == '9.9.9.9'


def test_record_delete(app):
    from app.services.dns_ownership_service import DnsOwnershipService as O
    O.record_write('cloudflare', 'z1', 'A', 'www.example.com', provider_record_id='R1', source='zone')
    assert O.record_delete('z1', provider_record_id='R1') == 1
    assert not O.owns('z1', provider_record_id='R1')


# ── guarded_upsert ───────────────────────────────────────────────────────────

def test_guarded_upsert_creates_and_records_ownership(app):
    from app.services.dns_ownership_service import DnsOwnershipService as O
    from app.services.dns.base import DnsRecordSpec
    client = FakeClient(existing_id=None, upsert_result={'success': True, 'record_id': 'CF1'})
    res = O.guarded_upsert(client, provider='cloudflare', provider_zone_id='z1',
                           spec=DnsRecordSpec('A', 'www.example.com', '1.2.3.4'),
                           source='zone', allow_foreign=False)
    assert res['success'] and res['record_id'] == 'CF1'
    assert O.owns('z1', provider_record_id='CF1')
    assert O.owns('z1', record_type='A', name='www.example.com')


def test_guarded_upsert_refuses_foreign(app):
    from app.services.dns_ownership_service import DnsOwnershipService as O
    from app.services.dns.base import DnsRecordSpec
    client = FakeClient(existing_id='FOREIGN')      # exists in CF, not in our ledger
    res = O.guarded_upsert(client, provider='cloudflare', provider_zone_id='z1',
                           spec=DnsRecordSpec('A', 'maria.example.com', '9.9.9.9'),
                           source='auto-dns', allow_foreign=False)
    assert res['success'] is False and res.get('conflict') is True
    assert 'upsert' not in _verbs(client)            # never wrote
    assert not O.owns('z1', record_type='A', name='maria.example.com')


def test_guarded_upsert_adopts_foreign_when_allowed(app):
    from app.services.dns_ownership_service import DnsOwnershipService as O
    from app.services.dns.base import DnsRecordSpec
    client = FakeClient(existing_id='ADOPT', upsert_result={'success': True, 'record_id': 'ADOPT'})
    res = O.guarded_upsert(client, provider='cloudflare', provider_zone_id='z1',
                           spec=DnsRecordSpec('A', 'www.example.com', '1.1.1.1'),
                           source='zone', allow_foreign=True)
    assert res['success']
    upsert_call = next(c for c in client.calls if c[0] == 'upsert')
    assert upsert_call[3] == 'ADOPT'                 # updated the existing record in place
    assert O.owns('z1', provider_record_id='ADOPT')


def test_guarded_upsert_updates_owned_without_conflict(app):
    from app.services.dns_ownership_service import DnsOwnershipService as O
    from app.services.dns.base import DnsRecordSpec
    O.record_write('cloudflare', 'z1', 'A', 'www.example.com', provider_record_id='OURS', source='zone')
    client = FakeClient(existing_id='OURS', upsert_result={'success': True, 'record_id': 'OURS'})
    res = O.guarded_upsert(client, provider='cloudflare', provider_zone_id='z1',
                           spec=DnsRecordSpec('A', 'www.example.com', '2.2.2.2'),
                           source='auto-dns', allow_foreign=False)
    assert res['success']                            # owned -> updates even on the strict path


# ── guarded_delete ───────────────────────────────────────────────────────────

def test_guarded_delete_owned_then_foreign(app):
    from app.services.dns_ownership_service import DnsOwnershipService as O
    O.record_write('cloudflare', 'z1', 'A', 'www.example.com', provider_record_id='R1', source='zone')

    owned = FakeClient()
    res = O.guarded_delete(owned, provider_zone_id='z1', record_type='A',
                           name='www.example.com', provider_record_id='R1')
    assert res['success'] and 'delete' in _verbs(owned)
    assert not O.owns('z1', provider_record_id='R1')         # ledger cleared

    foreign = FakeClient()
    res2 = O.guarded_delete(foreign, provider_zone_id='z1', record_type='A',
                            name='maria.example.com', provider_record_id='RX')
    assert res2.get('skipped') and 'delete' not in _verbs(foreign)


# ── mirror classification ────────────────────────────────────────────────────

def test_mirror_classifies_owned_vs_external(app, monkeypatch):
    from app import db
    from app.models.dns_zone import DNSZone
    from app.models.email import DNSProviderConfig
    from app.services.dns_provider_service import DNSProviderService
    from app.services.dns_zone_service import DNSZoneService
    from app.services.dns_ownership_service import DnsOwnershipService as O
    from app.services.dns import cloudflare as cf

    DNSProviderService.add_provider(name='cf', provider='cloudflare', api_key='tok')
    conn = DNSProviderConfig.query.filter_by(name='cf').first()
    zone = DNSZone(domain='example.com', provider='cloudflare',
                   provider_zone_id='Z', dns_provider_config_id=conn.id)
    db.session.add(zone)
    db.session.commit()

    O.record_write('cloudflare', 'Z', 'A', 'www.example.com', provider_record_id='MINE', source='zone')

    monkeypatch.setattr(cf.CloudflareClient, 'list_records', lambda self, zid: {'success': True, 'records': [
        {'id': 'MINE', 'type': 'A', 'name': 'www.example.com', 'content': '1.2.3.4',
         'ttl': 1, 'proxied': False, 'priority': None},
        {'id': 'THEIRS', 'type': 'A', 'name': 'maria.example.com', 'content': '5.6.7.8',
         'ttl': 1, 'proxied': False, 'priority': None},
    ]})

    out = DNSZoneService.list_provider_records(zone)
    assert out['success']
    by_name = {r['name']: r['managed_by'] for r in out['records']}
    assert by_name['www.example.com'] == 'serverkit'
    assert by_name['maria.example.com'] == 'external'
    assert out['counts'] == {'serverkit': 1, 'external': 1}


# ── end-to-end: the auto path refuses to clobber a foreign record ────────────

def test_provider_set_record_refuses_foreign(app, monkeypatch):
    from app.models.email import DNSProviderConfig
    from app.services.dns_provider_service import DNSProviderService
    from app.services.dns import cloudflare as cf

    DNSProviderService.add_provider(name='cf', provider='cloudflare', api_key='tok')
    cfg = DNSProviderConfig.query.filter_by(name='cf').first()

    # Cloudflare already has a record at this name that ServerKit didn't create.
    monkeypatch.setattr(cf.requests, 'get',
                        lambda url, headers=None, timeout=None: _Resp({'result': [{'id': 'FOREIGN'}]}))
    writes = {'n': 0}
    monkeypatch.setattr(cf.requests, 'post',
                        lambda *a, **k: (writes.update(n=writes['n'] + 1) or _Resp({'success': True})))
    monkeypatch.setattr(cf.requests, 'put',
                        lambda *a, **k: (writes.update(n=writes['n'] + 1) or _Resp({'success': True})))

    res = DNSProviderService.set_record(cfg.id, 'Z', 'A', 'maria.example.com', '1.2.3.4')
    assert res['success'] is False and res.get('conflict') is True
    assert writes['n'] == 0                          # never touched Cloudflare
