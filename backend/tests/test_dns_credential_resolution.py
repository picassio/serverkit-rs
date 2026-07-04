"""Proving tests for single-source DNS credentials (DNS unification #1).

The /dns Zones page and Dynamic DNS used to keep their *own* Cloudflare token in
``dns_zones.provider_config_json`` — separate from the connection in
Settings -> Connections (``DNSProviderConfig``). These tests prove the zone layer
now resolves credentials from the canonical store:

* an explicit link wins,
* otherwise the connection managing the domain is auto-discovered and the link is
  persisted (with the provider-side zone id),
* a legacy inline token still works until migrated,
* ``link_legacy_zones`` converges inline tokens onto encrypted connections
  (reusing a matching one, else minting), and
* Dynamic DNS rides all of this for free — an IP update syncs through the linked
  connection.
"""


class _Resp:
    def __init__(self, js):
        self._js = js

    def json(self):
        return self._js


def _add_connection(name='cf', token='tok'):
    """Create an encrypted Cloudflare connection and return the row."""
    from app.models.email import DNSProviderConfig
    from app.services.dns_provider_service import DNSProviderService
    DNSProviderService.add_provider(name=name, provider='cloudflare', api_key=token)
    return DNSProviderConfig.query.filter_by(name=name).first()


# ── _resolve_credential ──────────────────────────────────────────────────────

def test_resolve_prefers_linked_connection(app):
    from app import db
    from app.models.dns_zone import DNSZone
    from app.services.dns_zone_service import DNSZoneService

    conn = _add_connection(token='linked-token')
    zone = DNSZone(domain='example.com', provider='cloudflare',
                   provider_zone_id='zoneABC', dns_provider_config_id=conn.id)
    db.session.add(zone)
    db.session.commit()

    cred = DNSZoneService._resolve_credential(zone)
    assert cred.provider == 'cloudflare' and cred.token == 'linked-token'


def test_resolve_auto_discovers_and_persists_link(app, monkeypatch):
    from app import db
    from app.models.dns_zone import DNSZone
    from app.services.dns_zone_service import DNSZoneService
    from app.services.dns_provider_service import DNSProviderService

    conn = _add_connection(token='discovered')
    zone = DNSZone(domain='example.com', provider='cloudflare')  # no link, no zone id
    db.session.add(zone)
    db.session.commit()

    monkeypatch.setattr(DNSProviderService, 'find_zone_for_domain', classmethod(
        lambda cls, d: (conn, {'id': 'zoneXYZ', 'name': 'example.com'})))

    cred = DNSZoneService._resolve_credential(zone)
    assert cred.token == 'discovered'
    # the discovered link + provider zone id are persisted for next time
    assert zone.dns_provider_config_id == conn.id
    assert zone.provider_zone_id == 'zoneXYZ'


def test_resolve_falls_back_to_legacy_inline_token(app, monkeypatch):
    from app import db
    from app.models.dns_zone import DNSZone
    from app.services.dns_zone_service import DNSZoneService
    from app.services.dns_provider_service import DNSProviderService

    zone = DNSZone(domain='legacy.com', provider='cloudflare')
    zone.provider_config = {'api_token': 'legacy-token'}
    db.session.add(zone)
    db.session.commit()

    monkeypatch.setattr(DNSProviderService, 'find_zone_for_domain',
                        classmethod(lambda cls, d: (None, None)))

    cred = DNSZoneService._resolve_credential(zone)
    assert cred.token == 'legacy-token'
    assert zone.dns_provider_config_id is None  # nothing to link to yet


# ── link_legacy_zones ────────────────────────────────────────────────────────

def test_link_legacy_zones_mints_encrypted_connection(app):
    from app import db
    from app.models.dns_zone import DNSZone
    from app.models.email import DNSProviderConfig
    from app.services.dns_zone_service import DNSZoneService
    from app.utils.crypto import is_encrypted, decrypt_secret_safe

    zone = DNSZone(domain='example.com', provider='cloudflare', provider_zone_id='zoneABC')
    zone.provider_config = {'api_token': 'mint-me'}
    db.session.add(zone)
    db.session.commit()

    n = DNSZoneService.link_legacy_zones()
    assert n == 1

    conn = DNSProviderConfig.query.filter_by(provider='cloudflare').one()
    assert is_encrypted(conn.api_key) and decrypt_secret_safe(conn.api_key) == 'mint-me'
    assert zone.dns_provider_config_id == conn.id
    assert 'api_token' not in zone.provider_config       # plaintext token stripped

    # idempotent — the zone is now linked, so a second pass is a no-op
    assert DNSZoneService.link_legacy_zones() == 0


def test_link_legacy_zones_reuses_matching_connection(app):
    from app import db
    from app.models.dns_zone import DNSZone
    from app.models.email import DNSProviderConfig
    from app.services.dns_zone_service import DNSZoneService

    conn = _add_connection(name='existing', token='shared-token')
    zone = DNSZone(domain='example.com', provider='cloudflare', provider_zone_id='zoneABC')
    zone.provider_config = {'api_token': 'shared-token'}
    db.session.add(zone)
    db.session.commit()

    n = DNSZoneService.link_legacy_zones()
    assert n == 1
    # linked to the existing connection — no duplicate minted
    assert zone.dns_provider_config_id == conn.id
    assert DNSProviderConfig.query.filter_by(provider='cloudflare').count() == 1


# ── Dynamic DNS rides the unified credential path ────────────────────────────

def test_ddns_update_syncs_via_linked_connection(app, monkeypatch):
    """A DDNS IP update on a zone linked to a connection drives the shared
    Cloudflare client and captures the provider record id — no inline token."""
    from app import db
    from app.models.dns_zone import DNSZone, DNSRecord
    from app.services.ddns_service import DdnsService
    from app.services.dns import cloudflare as cf

    conn = _add_connection(token='tok')
    zone = DNSZone(domain='example.com', provider='cloudflare',
                   provider_zone_id='zoneABC', dns_provider_config_id=conn.id)
    db.session.add(zone)
    db.session.commit()

    host = DdnsService.create_host({'zone_id': zone.id, 'record_name': 'home'})

    monkeypatch.setattr(cf.requests, 'get',
                        lambda url, headers=None, timeout=None: _Resp({'result': []}))
    monkeypatch.setattr(cf.requests, 'post',
                        lambda url, headers=None, json=None, timeout=None:
                            _Resp({'success': True, 'result': {'id': 'CFREC'}}))

    status, updated = DdnsService.update_ip(host.token, '203.0.113.9')
    assert status == 'updated' and updated.last_ip == '203.0.113.9'

    rec = DNSRecord.query.filter_by(zone_id=zone.id, name='home', record_type='A').one()
    assert rec.content == '203.0.113.9'
    assert rec.provider_record_id == 'CFREC'      # synced through the linked connection
