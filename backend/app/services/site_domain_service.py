"""Resolve the public address of a managed site.

A bare published port (``127.0.0.1:8300``) is reachable on the box but is
useless as a public website URL. Instead every managed site is given a real
hostname ``<slug>.<base_domain>`` and the operator points a single wildcard DNS
record (``*.<base_domain>``) at the server — so a new site is reachable the
moment it is created, with no per-site DNS work.

The base domain is a one-time operator setting (``system_settings`` key
``sites_base_domain``), falling back to ``SITES_BASE_DOMAIN`` in config. In
development that defaults to ``lvh.me``, a public resolver that maps
``*.lvh.me -> 127.0.0.1``, so subdomain routing can be exercised locally with
zero DNS setup. When no base domain is configured the helpers return ``None``
and callers fall back to the legacy ``localhost:<port>`` behaviour.
"""
from flask import current_app

from app.models.system_settings import SystemSettings
from app.utils.slug import slugify as _slugify


class SiteDomainService:
    DEFAULT_BASE_DOMAIN = 'lvh.me'

    @staticmethod
    def _norm(value):
        return (str(value).strip().lstrip('.').lower()) if value else ''

    @classmethod
    def _default_row(cls):
        """The default SiteBaseDomain row, or None when the registry is empty
        (legacy single-setting / dev installs)."""
        try:
            from app.services.site_base_domain_service import SiteBaseDomainService
            return SiteBaseDomainService.default()
        except Exception:
            return None

    @classmethod
    def _row_for(cls, base):
        """The registry row for ``base`` (or the default row when base is None),
        or None when there's no matching/registered row."""
        try:
            from app.services.site_base_domain_service import SiteBaseDomainService
            if not base:
                return SiteBaseDomainService.default()
            return SiteBaseDomainService.get(base)
        except Exception:
            return None

    @classmethod
    def base_domain(cls):
        """The *default* base domain, or '' when site routing is not set up.

        Prefers the registry's default row; falls back to the legacy
        ``sites_base_domain`` setting, then the ``SITES_BASE_DOMAIN`` config
        default (so dev / not-yet-migrated installs keep working unchanged).
        """
        row = cls._default_row()
        if row:
            return row.domain
        val = SystemSettings.get('sites_base_domain')
        if val:
            return cls._norm(val)
        return cls._norm(current_app.config.get('SITES_BASE_DOMAIN'))

    @classmethod
    def all_base_domains(cls):
        """Every base domain a site may be published under (registry rows, else
        the single legacy base). Empty when site routing isn't configured."""
        try:
            from app.services.site_base_domain_service import SiteBaseDomainService
            rows = SiteBaseDomainService.list_rows()
            if rows:
                return [r.domain for r in rows]
        except Exception:
            pass
        base = cls.base_domain()
        return [base] if base else []

    @classmethod
    def resolve_base(cls, base=None):
        """The concrete base domain to publish under. An explicit ``base`` is
        honoured only when it's a registered/legacy base — an unknown value falls
        back to the default rather than publishing under an unmanaged domain."""
        if not base:
            return cls.base_domain()
        b = cls._norm(base)
        if b in cls.all_base_domains():
            return b
        return cls.base_domain()

    @classmethod
    def server_ip(cls):
        """Public IP that wildcard/custom A-records should point at (Phase 3)."""
        return SystemSettings.get('server_public_ip') or current_app.config.get('SERVER_PUBLIC_IP') or None

    @classmethod
    def panel_origin(cls):
        """Canonical public origin of the ServerKit panel, or None when no
        canonical domain is configured.

        Uses the persisted canonical_domain / canonical_https_enabled settings.
        Falls back to PUBLIC_URL / SERVERKIT_PUBLIC_URL env vars, then to the
        sites base domain. Returns None if nothing usable is configured.
        """
        domain = SystemSettings.get('canonical_domain')
        if domain:
            https = bool(SystemSettings.get('canonical_https_enabled', False))
            return f'https://{domain}' if https else f'http://{domain}'

        url = current_app.config.get('PUBLIC_URL') or current_app.config.get('SERVERKIT_PUBLIC_URL')
        if url:
            return url.rstrip('/')

        base = cls.base_domain()
        if base:
            return f'https://{base}' if cls.https_enabled() else f'http://{base}'

        return None

    @classmethod
    def https_enabled(cls, base=None):
        """True once the wildcard certificate for the given base (default when
        None) is set up, so its managed subdomains should be served over HTTPS.

        Reads the registry row when present; falls back to the legacy
        ``sites_https_enabled`` setting for the default/legacy base."""
        row = cls._row_for(base)
        if row is not None:
            return bool(row.https_enabled)
        if base is None or cls._norm(base) == cls.base_domain():
            return bool(SystemSettings.get('sites_https_enabled', False))
        return False

    @classmethod
    def wildcard_cert_paths(cls, base=None):
        """(fullchain, privkey) paths for a base's wildcard cert (default base
        when None), or (None, None) when no base domain is configured."""
        base = cls.resolve_base(base)
        if not base:
            return (None, None)
        return (f'/etc/letsencrypt/live/{base}/fullchain.pem',
                f'/etc/letsencrypt/live/{base}/privkey.pem')

    @classmethod
    def covering_base(cls, host):
        """The registered base domain whose wildcard covers ``host`` (host is the
        base or a subdomain of it), longest match wins, or None. Used to pick the
        right per-base certificate for a site's domains."""
        if not host:
            return None
        best = None
        for base in cls.all_base_domains():
            if host == base or host.endswith('.' + base):
                if best is None or len(base) > len(best):
                    best = base
        return best

    @classmethod
    def covers(cls, host):
        """Whether *any* registered base domain's wildcard cert covers ``host``."""
        return cls.covering_base(host) is not None

    @staticmethod
    def slugify(name):
        """Turn a site name into a DNS-safe label (a-z, 0-9, single dashes)."""
        return _slugify(name) or 'site'

    @classmethod
    def subdomain_for(cls, name, base=None):
        """``<slug>.<base>`` for a site name (default base when None), or ``None``
        when no base domain is configured (site routing disabled)."""
        base = cls.resolve_base(base)
        if not base:
            return None
        return f'{cls.slugify(name)}.{base}'

    @classmethod
    def site_url(cls, host, ssl=False):
        """Canonical URL for a host. HTTP for now; the wildcard-cert phase flips
        managed subdomains to HTTPS."""
        scheme = 'https' if ssl else 'http'
        return f'{scheme}://{host}'

    @classmethod
    def dns_mode(cls, base=None):
        """How a base's managed-site subdomains get their DNS (default base when
        None):

        * ``wildcard`` (default) — one ``*.<base>`` record covers every site, and
        * ``per-site`` — each site gets its own A record, auto-created via a
          connected provider.

        Reads the registry row when present; falls back to the legacy
        ``sites_dns_mode`` setting for the default/legacy base."""
        row = cls._row_for(base)
        if row is not None:
            return row.dns_mode if row.dns_mode in ('wildcard', 'per-site') else 'wildcard'
        val = (SystemSettings.get('sites_dns_mode') or '').strip().lower()
        return val if val in ('wildcard', 'per-site') else 'wildcard'

    @classmethod
    def ensure_site_dns(cls, host):
        """Auto-create a managed site's A record when its base is in ``per-site``
        mode (via a connected provider, ownership-guarded + logged). In ``wildcard``
        mode this is a no-op — the single ``*.<base>`` record already covers ``host``.
        Never raises; returns the provider result (or a ``skipped``/``no_server_ip``
        descriptor)."""
        base = cls.covering_base(host)
        if not host or cls.dns_mode(base) != 'per-site':
            return {'created': False, 'skipped': True, 'reason': 'wildcard'}
        ip = cls.server_ip()
        if not ip:
            return {'created': False, 'reason': 'no_server_ip',
                    'message': f'Set the server public IP to auto-create the {host} A record.'}
        try:
            from app.services.dns_provider_service import DNSProviderService
            return DNSProviderService.ensure_a_record(host, ip)
        except Exception as e:
            return {'created': False, 'reason': 'error', 'error': str(e)}

    @classmethod
    def panel_host(cls):
        """Hostname the ServerKit panel *itself* is served on, from an explicitly
        configured panel domain (``canonical_domain`` setting, else the
        ``PUBLIC_URL`` / ``SERVERKIT_PUBLIC_URL`` env), or ``None``.

        Deliberately does NOT use ``panel_origin``'s base-domain fallback — that
        would make the panel host equal the sites base domain and defeat both the
        subdomain-vs-apex suggestion and the base/panel overlap check.
        """
        domain = SystemSettings.get('canonical_domain')
        if not domain:
            domain = (current_app.config.get('PUBLIC_URL')
                      or current_app.config.get('SERVERKIT_PUBLIC_URL'))
        if not domain:
            return None
        host = str(domain).split('://', 1)[-1].split('/', 1)[0].split(':', 1)[0]
        return host.strip().lower().strip('.') or None

    @classmethod
    def suggested_base_domain(cls):
        """A sensible ``sites_base_domain`` to suggest, derived from the panel's
        own domain so the recommendation fits the install shape:

        * apex install ``example.com`` → ``apps.example.com`` (keeps the apex for
          the panel, scopes the site wildcard under a dedicated label), and
        * subdomain install ``panel.example.com`` → ``apps.example.com`` (a
          sibling label, so ``*.apps.example.com`` never collides with the
          panel's own ``panel`` record).

        Returns ``None`` when the panel domain is unknown (nothing to derive from).
        """
        host = cls.panel_host()
        if not host or host in ('localhost',) or host.replace('.', '').isdigit():
            return None
        parts = host.split('.')
        # A subdomain install (3+ labels) → sibling under the parent zone; an
        # apex install (2 labels) → a dedicated label under the apex.
        parent = '.'.join(parts[1:]) if len(parts) >= 3 else host
        return f'apps.{parent}'

    @classmethod
    def base_domain_overlaps_panel(cls):
        """When the managed-sites wildcard (``*.<base>``) would also capture the
        panel's own hostname, return a human explanation; else ``None``.

        Two overlaps matter:

        * **panel host == base** — the wildcard setup's apex ``<base>`` A record
          repoints the panel's own domain at the site server, and every
          ``*.<base>`` becomes a managed site.
        * **panel host is a direct (single-label) child of base**
          (``panel.example.com`` under ``example.com``) — ``*.<base>`` matches the
          panel host, so it keeps working only while its explicit ``<panel>``
          record out-specifies the wildcard; drop that record and the panel
          silently falls into the site-serving nginx.

        A *deeper* descendant (``a.b.<base>``) is safe — a wildcard is single-label.
        """
        base = cls.base_domain()
        panel = cls.panel_host()
        if not base or not panel or panel == 'localhost' or panel.replace('.', '').isdigit():
            return None
        if panel == base:
            return (f'The panel is served at {panel} — the same domain set as the '
                    f'managed-sites base. Every *.{base} then becomes a site and the '
                    f'{base} record is managed for you. Use a dedicated base such as '
                    f'apps.{base} to keep the panel separate.')
        if panel.endswith('.' + base):
            label = panel[: -(len(base) + 1)]
            if '.' not in label:  # direct child only — *.<base> is single-label
                return (f'The panel host {panel} sits directly under the managed-sites '
                        f'base {base}, so the *.{base} wildcard also matches it. The '
                        f'panel keeps working only while its own {panel} DNS record '
                        f'out-specifies the wildcard — pick a base that does not '
                        f'contain the panel host (e.g. apps.{base}) to avoid this.')
        return None

    @classmethod
    def publishing_gaps(cls):
        """The managed-sites publishing config gaps that are open *right now*, as
        a list of ``{code, event, message}`` (empty when publishing is fully set
        up). Only surfaces gaps that are actionable given what IS configured — the
        HTTPS and server-IP gaps are meaningless before a base domain exists, so a
        missing base domain short-circuits the rest.
        """
        base = cls.base_domain()
        if not base:
            suggestion = cls.suggested_base_domain()
            eg = f' (e.g. {suggestion})' if suggestion else ''
            return [{
                'code': 'no_base_domain',
                'event': 'sites.publish.no_base_domain',
                'message': (
                    'New sites are only reachable at localhost:<port>. Set a '
                    f'managed-sites base domain{eg} in Settings → Managed Sites and '
                    'point a wildcard record (*.<domain>) at this server, so every '
                    'site is published at <name>.<domain>.'),
            }]

        gaps = []
        overlap = cls.base_domain_overlaps_panel()
        if overlap:
            gaps.append({
                'code': 'base_overlaps_panel',
                'event': 'sites.publish.base_overlaps_panel',
                'message': overlap,
            })
        if not cls.https_enabled():
            gaps.append({
                'code': 'http_only',
                'event': 'sites.publish.http_only',
                'message': (
                    f'Sites are published over HTTP at <name>.{base}. Enabling '
                    f'wildcard HTTPS (Settings → Managed Sites) serves them over TLS '
                    f'from a *.{base} certificate — optional, but recommended for a '
                    f'public site.'),
            })
        if cls.dns_mode() == 'per-site' and not cls.server_ip():
            gaps.append({
                'code': 'no_server_ip',
                'event': 'sites.publish.no_server_ip',
                'message': (
                    f"DNS mode is per-site but no server public IP is set, so each "
                    f"site's A record under {base} can't be auto-created. Set the "
                    'server public IP in Settings, or switch to wildcard DNS mode.'),
            })
        return gaps

    @classmethod
    def _has_open_gap_notice(cls, event_key):
        """True when an admin already has an *unread* in-app nudge for this event,
        so the same gap is nudged once — not on every site create."""
        try:
            from app import db
            from app.notifications.models import Notification, NotificationDelivery
            return db.session.query(NotificationDelivery.id).join(
                Notification, NotificationDelivery.notification_id == Notification.id
            ).filter(
                Notification.event_key == event_key,
                NotificationDelivery.channel == NotificationDelivery.CHANNEL_INAPP,
                NotificationDelivery.read_at.is_(None),
            ).first() is not None
        except Exception:
            return False

    @classmethod
    def notify_publishing_gaps(cls):
        """Best-effort: drop an in-app nudge to admins for each open publishing
        gap, deduped against an already-open unread nudge for the same gap. Never
        raises — a nudge must never break the create flow that triggered it.
        Returns ``{'sent': n}``."""
        try:
            gaps = cls.publishing_gaps()
        except Exception:
            return {'sent': 0}
        sent = 0
        for gap in gaps:
            try:
                if cls._has_open_gap_notice(gap['event']):
                    continue
                from app.plugins_sdk import notify
                notify.send(gap['event'], to='admins',
                            data={'message': gap['message'], 'summary': gap['message']})
                sent += 1
            except Exception:
                continue
        return {'sent': sent}

    @classmethod
    def _vhost_create_kwargs(cls, app, domains, ssl_cert, ssl_key, force_type=None):
        """Build ``NginxService.create_site(**kwargs)`` for ``app``, or
        ``(None, reason)`` when the app type can't be served by host nginx or is
        missing what it needs (a port for proxied apps, a root for served ones).

        ``force_type`` overrides the ``app_type`` → template choice — a managed
        WordPress site always proxies to its container port even though the row
        says ``wordpress`` (whose stock template is php-fpm, not a proxy).
        """
        t = (force_type or app.app_type or '').lower()
        base = dict(name=app.name, domains=domains, ssl_cert=ssl_cert, ssl_key=ssl_key)
        # Reverse-proxy to a local container/app port.
        if t in ('docker', 'wordpress'):
            if not app.port:
                return None, f'{t} app has no published port to route to.'
            return dict(base, app_type='docker', port=app.port), None
        if t in ('flask', 'django', 'python'):
            if not app.port:
                return None, f'{t} app has no published port to route to.'
            return dict(base, app_type=t, root_path=app.root_path or '', port=app.port), None
        # Serve from a filesystem root.
        if t == 'php':
            if not app.root_path:
                return None, 'php app has no root path to serve.'
            return dict(base, app_type='php', root_path=app.root_path,
                        php_version=(getattr(app, 'php_version', None) or '8.2')), None
        if t == 'static':
            if not app.root_path:
                return None, 'static app has no root path to serve.'
            return dict(base, app_type='static', root_path=app.root_path), None
        return None, f"app type '{t}' cannot be published via host nginx."

    @classmethod
    def write_app_vhost(cls, app, force_type=None):
        """(Re)write and enable the host-nginx vhost publishing ``app`` at every
        one of its Domain rows (``server_name`` = all domains).

        Handles all host-nginx app types: docker/wordpress and
        python/flask/django reverse-proxy to the app's port; php/static serve a
        filesystem root. Serves a base's wildcard cert when every domain is a
        subdomain of that *same* base and its HTTPS is enabled (custom domains
        bring their own cert). Best-effort — never raises; returns ``{'nginx',
        'warning'}`` (``nginx`` is ``None`` when nothing was written).
        """
        from app.models.domain import Domain
        from app.services.nginx_service import NginxService

        domains = [d.name for d in Domain.query.filter_by(application_id=app.id).all()]
        if not domains:
            return {'nginx': None, 'warning': None}

        # Pick a wildcard cert only when all domains sit under one base (whichever
        # base covers them) and that base has HTTPS set up.
        ssl_cert = ssl_key = None
        cover = cls.covering_base(domains[0])
        if cover and cls.https_enabled(cover) and all(cls.covering_base(d) == cover for d in domains):
            ssl_cert, ssl_key = cls.wildcard_cert_paths(cover)

        kwargs, reason = cls._vhost_create_kwargs(app, domains, ssl_cert, ssl_key, force_type)
        if reason:
            return {'nginx': None, 'warning': reason}

        try:
            res = NginxService.create_site(**kwargs)
        except Exception as e:
            return {'nginx': None, 'warning': str(e)}
        if not res.get('success'):
            return {'nginx': res, 'warning': f"nginx vhost not created: {res.get('error')}"}
        en = NginxService.enable_site(app.name)
        if not en.get('success'):
            return {'nginx': res, 'warning': f"vhost written but not enabled: {en.get('error')}"}
        return {'nginx': res, 'warning': None}

    @classmethod
    def give_subdomain(cls, app, label=None, base=None):
        """One-click 'give this app a subdomain': publish ``app`` at
        ``<label>.<base>`` (label defaults to the app-name slug; ``base`` defaults
        to the default registered base domain). Creates the primary Domain row,
        (re)writes its nginx vhost, and — in per-site DNS mode — auto-creates the A
        record (wildcard mode relies on ``*.<base>``).

        Returns ``{success, host, url, dns, nginx, warning}`` or
        ``{success: False, error}``.
        """
        from app import db
        from app.models.domain import Domain

        base = cls.resolve_base(base)
        if not base:
            return {'success': False, 'error': 'Set the managed-sites base domain first (Settings).'}

        host = f'{cls.slugify(label or app.name)}.{base}'
        existing = Domain.query.filter_by(name=host).first()
        if existing and existing.application_id != app.id:
            return {'success': False, 'error': f'{host} is already used by another app.'}

        try:
            if not existing:
                make_primary = Domain.query.filter_by(
                    application_id=app.id, is_primary=True).first() is None
                if make_primary:
                    Domain.query.filter_by(application_id=app.id, is_primary=True).update(
                        {'is_primary': False})
                db.session.add(Domain(name=host, is_primary=make_primary, application_id=app.id))
                db.session.commit()
        except Exception as e:
            db.session.rollback()
            return {'success': False, 'error': f'Could not record domain: {e}'}

        # Publish the site: (re)write + enable its host-nginx vhost. Works for
        # every host-nginx app type (docker/wordpress/python proxy to the app
        # port; php/static serve a root), not just docker — a non-routable or
        # misconfigured app degrades to a warning rather than failing the publish.
        v = cls.write_app_vhost(app)
        nginx = v.get('nginx')
        warning = v.get('warning')

        dns = cls.ensure_site_dns(host)
        if dns and not dns.get('skipped') and not dns.get('created') and dns.get('message'):
            warning = (warning + '; ' + dns['message']) if warning else dns['message']

        # Nudge admins about any remaining publishing-config gaps (HTTP-only,
        # per-site DNS without a server IP). Best-effort, deduped, never fatal.
        cls.notify_publishing_gaps()

        return {'success': True, 'host': host,
                'url': cls.site_url(host, ssl=cls.https_enabled(base) and cls.covers(host)),
                'dns': dns, 'nginx': nginx, 'warning': warning}
