"""Prove the Cloudflare zone-ops extraction (Phase 5, #36).

Zone-ops (the /api/v1/cloudflare blueprint + CloudflareService) physically moved
into the bundled, default-installed serverkit-cloudflare-ops extension (a flagship,
like WordPress). The DNS layer stays core: the moved service borrows the single
core CloudflareClient via DNSZoneService — there is no duplicate Cloudflare client.
"""
import importlib

import pytest

from app.models.plugin import InstalledPlugin
from app.services import cloudflare_ops_bridge

SLUG = 'serverkit-cloudflare-ops'


def test_cloudflare_service_left_core():
    """The zone-ops service is gone from core after extraction."""
    with pytest.raises(ModuleNotFoundError):
        importlib.import_module('app.services.cloudflare_service')


def test_dns_client_stays_core_single_source():
    """The one Cloudflare API client remains core (no duplicate in the extension)."""
    from app.services.dns import CloudflareClient
    assert CloudflareClient.__module__ == 'app.services.dns.cloudflare'


def test_bridge_resolves_service_from_extension():
    CloudflareService = cloudflare_ops_bridge.cloudflare_service()
    assert CloudflareService.__name__ == 'CloudflareService'
    assert CloudflareService.__module__.startswith(f'app.plugins.{SLUG}')
    # And it does not vendor its own client — it imports the core one lazily.
    import inspect
    src = inspect.getsource(CloudflareService)
    assert 'from app.services.dns import CloudflareClient' in src
    assert 'DNSZoneService._resolve_credential' in src


def test_cloudflare_seeded_as_flagship(app):
    plugin = InstalledPlugin.query.filter_by(slug=SLUG).first()
    assert plugin is not None, 'Cloudflare zone-ops flagship should be seeded on boot'
    assert plugin.status == InstalledPlugin.STATUS_ACTIVE
    assert plugin.url_prefix == '/api/v1/cloudflare'


def test_cloudflare_blueprint_registered_from_extension(app):
    rules = [r.rule for r in app.url_map.iter_rules()]
    assert any(r.startswith('/api/v1/cloudflare/zones') for r in rules), \
        'Cloudflare zone-ops API should be registered from the extension'


def test_cloudflare_api_reachable_not_404(app, client, auth_headers):
    """The extension blueprint is live (not 404) and the status guard passes for
    the active flagship (not 503). It may 400/502 without a real CF zone — we only
    assert the route is wired."""
    resp = client.get('/api/v1/cloudflare/zones/1/settings', headers=auth_headers)
    assert resp.status_code not in (404, 503), resp.status_code
