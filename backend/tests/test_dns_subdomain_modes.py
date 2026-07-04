"""Proving tests for per-base-domain subdomain DNS modes (Phase 3).

A managed base domain runs in one of two modes:
* ``wildcard`` (default) — one ``*.<base>`` record covers every site, so creating a
  site does no per-site DNS, and
* ``per-site`` — each new site gets its own A record auto-created via a connected
  provider (through the ownership-guarded, logged write path).
"""


def _set(key, value):
    from app import db
    from app.services.settings_service import SettingsService
    SettingsService.set(key, value)
    db.session.commit()


# ── mode setting ─────────────────────────────────────────────────────────────

def test_dns_mode_defaults_to_wildcard(app):
    from app.services.site_domain_service import SiteDomainService
    assert SiteDomainService.dns_mode() == 'wildcard'


def test_dns_mode_reads_per_site(app):
    from app.services.site_domain_service import SiteDomainService
    _set('sites_dns_mode', 'per-site')
    assert SiteDomainService.dns_mode() == 'per-site'


def test_dns_mode_rejects_garbage(app):
    from app.services.site_domain_service import SiteDomainService
    _set('sites_dns_mode', 'nonsense')
    assert SiteDomainService.dns_mode() == 'wildcard'


# ── ensure_site_dns ──────────────────────────────────────────────────────────

def test_ensure_site_dns_is_noop_in_wildcard_mode(app, monkeypatch):
    from app.services.site_domain_service import SiteDomainService
    from app.services.dns_provider_service import DNSProviderService

    called = {'n': 0}
    monkeypatch.setattr(DNSProviderService, 'ensure_a_record',
                        classmethod(lambda cls, h, ip: called.update(n=called['n'] + 1)))
    res = SiteDomainService.ensure_site_dns('blog.example.com')
    assert res.get('skipped') and res.get('reason') == 'wildcard'
    assert called['n'] == 0           # wildcard already covers it; no provider call


def test_ensure_site_dns_creates_record_in_per_site_mode(app, monkeypatch):
    from app.services.site_domain_service import SiteDomainService
    from app.services.dns_provider_service import DNSProviderService

    _set('sites_dns_mode', 'per-site')
    _set('server_public_ip', '203.0.113.7')

    seen = {}
    monkeypatch.setattr(DNSProviderService, 'ensure_a_record', classmethod(
        lambda cls, host, ip: (seen.update(host=host, ip=ip) or {'created': True, 'record': {}})))

    res = SiteDomainService.ensure_site_dns('blog.example.com')
    assert res.get('created') is True
    assert seen == {'host': 'blog.example.com', 'ip': '203.0.113.7'}


def test_ensure_site_dns_per_site_without_ip(app):
    from app.services.site_domain_service import SiteDomainService
    _set('sites_dns_mode', 'per-site')
    res = SiteDomainService.ensure_site_dns('blog.example.com')
    assert res.get('created') is False and res.get('reason') == 'no_server_ip'


# ── status surfaces the mode ─────────────────────────────────────────────────

def test_sites_https_status_reports_mode(app):
    from app.services.sites_https_service import SitesHttpsService
    _set('sites_dns_mode', 'per-site')
    status = SitesHttpsService.status()
    assert status['dns_mode'] == 'per-site'
