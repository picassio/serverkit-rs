"""Proving tests for managed-sites publishing-readiness nudges.

When a site is created but the base-domain / DNS / HTTPS config is only partly
set up, admins get an in-app nudge to finish it — deduped so it fires once, not
on every create. The suggested base domain is derived from the panel's own
domain so it fits a subdomain vs apex install.
"""


def _set(key, value):
    from app import db
    from app.services.settings_service import SettingsService
    SettingsService.set(key, value)
    db.session.commit()


def _mk_admin(username='pg-admin'):
    from app import db
    from app.models import User
    from werkzeug.security import generate_password_hash
    u = User(email=f'{username}@pg.local', username=username,
             password_hash=generate_password_hash('x'), role=User.ROLE_ADMIN, is_active=True)
    db.session.add(u)
    db.session.commit()
    return u


def _clear_base(app, monkeypatch):
    # TestingConfig pins SITES_BASE_DOMAIN=lvh.me; clear both layers.
    monkeypatch.setitem(app.config, 'SITES_BASE_DOMAIN', '')
    _set('sites_base_domain', '')


# ── publishing_gaps ──────────────────────────────────────────────────────────

def test_gap_no_base_domain(app, monkeypatch):
    from app.services.site_domain_service import SiteDomainService
    _clear_base(app, monkeypatch)
    gaps = SiteDomainService.publishing_gaps()
    assert [g['code'] for g in gaps] == ['no_base_domain']
    # No other gaps are computed until a base domain exists.
    assert gaps[0]['event'] == 'sites.publish.no_base_domain'


def test_gap_http_only_when_base_set_no_https(app, monkeypatch):
    from app.services.site_domain_service import SiteDomainService
    _set('sites_base_domain', 'apps.example.com')          # default mode = wildcard
    _set('sites_https_enabled', False)
    gaps = SiteDomainService.publishing_gaps()
    assert [g['code'] for g in gaps] == ['http_only']


def test_gap_per_site_without_server_ip(app, monkeypatch):
    from app.services.site_domain_service import SiteDomainService
    _set('sites_base_domain', 'apps.example.com')
    _set('sites_https_enabled', False)
    _set('sites_dns_mode', 'per-site')
    _set('server_public_ip', '')
    codes = [g['code'] for g in SiteDomainService.publishing_gaps()]
    assert 'no_server_ip' in codes and 'http_only' in codes


def test_no_gaps_when_fully_set_up(app, monkeypatch):
    from app.services.site_domain_service import SiteDomainService
    _set('sites_base_domain', 'apps.example.com')
    _set('sites_https_enabled', True)                       # wildcard mode, https on
    assert SiteDomainService.publishing_gaps() == []


# ── panel-domain-aware suggestion (subdomain vs apex install) ────────────────

def test_suggested_base_domain_apex_install(app, monkeypatch):
    from app.services.site_domain_service import SiteDomainService
    _set('canonical_domain', 'example.com')                # panel on the apex
    assert SiteDomainService.suggested_base_domain() == 'apps.example.com'


def test_suggested_base_domain_subdomain_install(app, monkeypatch):
    from app.services.site_domain_service import SiteDomainService
    _set('canonical_domain', 'panel.acme.co.uk')           # panel on a subdomain
    # Sibling label under the parent zone, so *.apps.* never collides with `panel`.
    assert SiteDomainService.suggested_base_domain() == 'apps.acme.co.uk'


def test_no_base_domain_message_includes_suggestion(app, monkeypatch):
    from app.services.site_domain_service import SiteDomainService
    _clear_base(app, monkeypatch)
    _set('canonical_domain', 'panel.example.com')
    msg = SiteDomainService.publishing_gaps()[0]['message']
    assert 'apps.example.com' in msg


# ── base-domain / panel-domain overlap (the wildcard footgun) ────────────────

def test_overlap_when_base_equals_panel(app, monkeypatch):
    from app.services.site_domain_service import SiteDomainService
    _set('sites_base_domain', 'example.com')
    _set('canonical_domain', 'example.com')                # panel IS the base
    assert SiteDomainService.base_domain_overlaps_panel() is not None
    assert 'base_overlaps_panel' in [g['code'] for g in SiteDomainService.publishing_gaps()]


def test_overlap_when_panel_is_direct_child_of_base(app, monkeypatch):
    from app.services.site_domain_service import SiteDomainService
    _set('sites_base_domain', 'example.com')
    _set('canonical_domain', 'panel.example.com')          # *.example.com matches panel
    msg = SiteDomainService.base_domain_overlaps_panel()
    assert msg and 'panel.example.com' in msg


def test_no_overlap_for_deep_descendant(app, monkeypatch):
    from app.services.site_domain_service import SiteDomainService
    _set('sites_base_domain', 'example.com')
    _set('canonical_domain', 'a.b.example.com')            # wildcard is single-label
    assert SiteDomainService.base_domain_overlaps_panel() is None


def test_no_overlap_when_base_is_scoped_label(app, monkeypatch):
    from app.services.site_domain_service import SiteDomainService
    _set('sites_base_domain', 'apps.example.com')          # the recommended shape
    _set('canonical_domain', 'panel.example.com')
    assert SiteDomainService.base_domain_overlaps_panel() is None
    assert 'base_overlaps_panel' not in [g['code'] for g in SiteDomainService.publishing_gaps()]


# ── notify_publishing_gaps (in-app, deduped) ─────────────────────────────────

def test_notify_sends_once_then_dedupes(app, monkeypatch):
    from app.services.site_domain_service import SiteDomainService
    from app.notifications.service import NotificationBusService
    admin = _mk_admin()
    _clear_base(app, monkeypatch)

    first = SiteDomainService.notify_publishing_gaps()
    assert first['sent'] == 1
    assert NotificationBusService.unread_count(admin.id) == 1

    # Second create with the same open gap must not stack another nudge.
    second = SiteDomainService.notify_publishing_gaps()
    assert second['sent'] == 0
    assert NotificationBusService.unread_count(admin.id) == 1


def test_notify_sends_nothing_when_ready(app, monkeypatch):
    from app.services.site_domain_service import SiteDomainService
    from app.notifications.service import NotificationBusService
    admin = _mk_admin(username='pg-admin2')
    _set('sites_base_domain', 'apps.example.com')
    _set('sites_https_enabled', True)
    assert SiteDomainService.notify_publishing_gaps() == {'sent': 0}
    assert NotificationBusService.unread_count(admin.id) == 0
