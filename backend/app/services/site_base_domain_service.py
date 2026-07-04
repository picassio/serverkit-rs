"""CRUD + resolution for the managed-site base-domain registry.

The registry (``site_base_domains``) lets managed sites be published under any of
several base domains (``example.com``, ``toto.com``, …), each with its own DNS
provider + wildcard cert. Exactly one row is the default. This service owns the
writes; SiteDomainService reads through it for routing/cert decisions.

Back-compat: installs that only ever had the single ``sites_base_domain`` setting
have zero rows. ``ensure_seeded`` materialises that legacy value into the default
row the moment a second domain is registered, so the two never disagree.
"""
import re

from app import db
from app.models.site_base_domain import SiteBaseDomain
from app.models.system_settings import SystemSettings

_DOMAIN_RE = re.compile(
    r'^(?:[a-z0-9](?:[a-z0-9\-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$'
)


def normalize_domain(value):
    """Lowercase, trim, drop a leading dot. Returns '' for falsy input."""
    return (str(value).strip().lstrip('.').lower()) if value else ''


class SiteBaseDomainService:

    @staticmethod
    def is_valid_domain(domain):
        d = normalize_domain(domain)
        return bool(d) and len(d) <= 253 and bool(_DOMAIN_RE.match(d))

    # ------------------------------------------------------------------
    # Seeding / back-compat
    # ------------------------------------------------------------------
    @classmethod
    def ensure_seeded(cls):
        """If the registry is empty but a legacy ``sites_base_domain`` setting
        exists, create the default row from the legacy sites settings so the
        registry and the old single-domain settings agree. No-op otherwise (and
        deliberately does NOT seed the dev-only ``SITES_BASE_DOMAIN`` config —
        only a persisted operator setting)."""
        if SiteBaseDomain.query.first() is not None:
            return None
        legacy = normalize_domain(SystemSettings.get('sites_base_domain'))
        if not legacy:
            return None
        mode = (SystemSettings.get('sites_dns_mode') or 'wildcard')
        row = SiteBaseDomain(
            domain=legacy,
            is_default=True,
            dns_mode=mode if mode in SiteBaseDomain.DNS_MODES else 'wildcard',
            https_enabled=bool(SystemSettings.get('sites_https_enabled', False)),
        )
        db.session.add(row)
        db.session.commit()
        return row

    # ------------------------------------------------------------------
    # Reads
    # ------------------------------------------------------------------
    @classmethod
    def list_rows(cls):
        return SiteBaseDomain.query.order_by(
            SiteBaseDomain.is_default.desc(), SiteBaseDomain.domain.asc()).all()

    @classmethod
    def get(cls, domain):
        return SiteBaseDomain.query.filter_by(domain=normalize_domain(domain)).first()

    @classmethod
    def default(cls):
        """The default row (explicit flag, else the oldest), or None."""
        return (SiteBaseDomain.query.filter_by(is_default=True).first()
                or SiteBaseDomain.query.order_by(SiteBaseDomain.id.asc()).first())

    # ------------------------------------------------------------------
    # Writes
    # ------------------------------------------------------------------
    @classmethod
    def add(cls, domain, dns_provider_config_id=None, dns_mode='wildcard', make_default=False):
        """Register a base domain. Seeds the legacy default first, so the first
        added domain doesn't silently become the sole/default and orphan the old
        single-domain config. Returns ``{'success', 'base'|'error'}``."""
        d = normalize_domain(domain)
        if not cls.is_valid_domain(d):
            return {'success': False, 'error': f'Invalid domain: {domain}'}
        cls.ensure_seeded()
        if cls.get(d):
            return {'success': False, 'error': f'{d} is already registered.'}
        mode = dns_mode if dns_mode in SiteBaseDomain.DNS_MODES else 'wildcard'
        first = SiteBaseDomain.query.first() is None
        row = SiteBaseDomain(
            domain=d, dns_mode=mode, dns_provider_config_id=dns_provider_config_id,
            is_default=bool(make_default or first),
        )
        db.session.add(row)
        db.session.flush()
        if row.is_default:
            cls._demote_others(row.id)
        db.session.commit()
        return {'success': True, 'base': row.to_dict()}

    @classmethod
    def set_default(cls, domain):
        row = cls.get(domain)
        if not row:
            return {'success': False, 'error': f'{domain} is not registered.'}
        row.is_default = True
        cls._demote_others(row.id)
        db.session.commit()
        return {'success': True, 'base': row.to_dict()}

    @classmethod
    def update(cls, domain, dns_mode=None, dns_provider_config_id=None, https_enabled=None):
        row = cls.get(domain)
        if not row:
            return {'success': False, 'error': f'{domain} is not registered.'}
        if dns_mode is not None and dns_mode in SiteBaseDomain.DNS_MODES:
            row.dns_mode = dns_mode
        if dns_provider_config_id is not None:
            row.dns_provider_config_id = dns_provider_config_id or None
        if https_enabled is not None:
            row.https_enabled = bool(https_enabled)
        db.session.commit()
        return {'success': True, 'base': row.to_dict()}

    @classmethod
    def remove(cls, domain):
        row = cls.get(domain)
        if not row:
            return {'success': False, 'error': f'{domain} is not registered.'}
        was_default = row.is_default
        db.session.delete(row)
        db.session.flush()
        if was_default:
            # Promote the oldest survivor so there's always a default.
            successor = SiteBaseDomain.query.order_by(SiteBaseDomain.id.asc()).first()
            if successor:
                successor.is_default = True
        db.session.commit()
        return {'success': True}

    @staticmethod
    def _demote_others(keep_id):
        SiteBaseDomain.query.filter(SiteBaseDomain.id != keep_id).update(
            {'is_default': False}, synchronize_session=False)
