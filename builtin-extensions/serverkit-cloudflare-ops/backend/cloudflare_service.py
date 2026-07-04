"""Cloudflare zone operations beyond DNS records.

ServerKit already connects Cloudflare as a DNS provider (``DNSProviderConfig`` +
the shared :class:`~app.services.dns.cloudflare.CloudflareClient`). This service
builds the *operations* surface on top of that same connection — starting with
zone settings (SSL/TLS, Speed, Caching, Security) and a one-click hardening
preset — so auth, encryption-at-rest, and credential resolution are reused, not
re-implemented.

A zone is addressed by its ServerKit ``DNSZone`` id (the same integer the rest of
the ``/dns`` API uses); credential + Cloudflare zone id are resolved server-side
via :meth:`DNSZoneService._resolve_credential`, the canonical resolver.
"""
import logging
import re

logger = logging.getLogger(__name__)


class CloudflareError(Exception):
    """A caller-facing problem resolving a zone (not found, not Cloudflare, no
    connected credential). Mapped to a 400 by the API layer."""


class CloudflareService:
    """Zone settings + hardening on a connected Cloudflare zone."""

    # Curated subset of Cloudflare zone settings ServerKit surfaces, grouped for
    # the UI. Each setting: ``id`` (Cloudflare setting id), ``label``, ``type``
    # (toggle | select | hsts) and, for selects, ``options`` ({value, label}).
    # The page renders straight from this metadata; current values + the
    # ``editable`` (plan-gating) flag come from the live settings response.
    SETTING_GROUPS = [
        {
            'key': 'ssl',
            'label': 'SSL/TLS',
            'settings': [
                {'id': 'ssl', 'label': 'SSL/TLS encryption mode', 'type': 'select',
                 'help': 'How Cloudflare connects to your origin. "Full (strict)" '
                         'is the most secure and requires a valid origin certificate.',
                 'options': [
                     {'value': 'off', 'label': 'Off (not secure)'},
                     {'value': 'flexible', 'label': 'Flexible'},
                     {'value': 'full', 'label': 'Full'},
                     {'value': 'strict', 'label': 'Full (strict)'},
                 ]},
                {'id': 'always_use_https', 'label': 'Always use HTTPS', 'type': 'toggle',
                 'help': 'Redirect every HTTP request to HTTPS.'},
                {'id': 'automatic_https_rewrites', 'label': 'Automatic HTTPS rewrites',
                 'type': 'toggle',
                 'help': 'Rewrite insecure http:// links to https:// to avoid mixed content.'},
                {'id': 'min_tls_version', 'label': 'Minimum TLS version', 'type': 'select',
                 'options': [
                     {'value': '1.0', 'label': 'TLS 1.0'},
                     {'value': '1.1', 'label': 'TLS 1.1'},
                     {'value': '1.2', 'label': 'TLS 1.2 (recommended)'},
                     {'value': '1.3', 'label': 'TLS 1.3'},
                 ]},
                {'id': 'tls_1_3', 'label': 'TLS 1.3', 'type': 'toggle',
                 'help': 'Enable the latest, fastest TLS version.'},
                {'id': 'security_header', 'label': 'HTTP Strict Transport Security (HSTS)',
                 'type': 'hsts',
                 'help': 'Tell browsers to only ever connect over HTTPS. Enable only once '
                         'HTTPS works everywhere — it is hard to undo before max-age expires.'},
            ],
        },
        {
            'key': 'speed',
            'label': 'Speed',
            'settings': [
                {'id': 'brotli', 'label': 'Brotli compression', 'type': 'toggle',
                 'help': 'Compress responses with Brotli for supporting browsers.'},
                {'id': 'early_hints', 'label': 'Early Hints', 'type': 'toggle',
                 'help': 'Send 103 Early Hints so browsers can preload assets sooner.'},
                {'id': 'http3', 'label': 'HTTP/3 (with QUIC)', 'type': 'toggle'},
            ],
        },
        {
            'key': 'caching',
            'label': 'Caching',
            'settings': [
                {'id': 'cache_level', 'label': 'Caching level', 'type': 'select',
                 'options': [
                     {'value': 'bypass', 'label': 'Bypass'},
                     {'value': 'basic', 'label': 'Basic'},
                     {'value': 'simplified', 'label': 'Simplified'},
                     {'value': 'aggressive', 'label': 'Aggressive (recommended)'},
                     {'value': 'cache_everything', 'label': 'Cache everything'},
                 ]},
                {'id': 'browser_cache_ttl', 'label': 'Browser cache TTL', 'type': 'select',
                 'options': [
                     {'value': 0, 'label': 'Respect existing headers'},
                     {'value': 1800, 'label': '30 minutes'},
                     {'value': 3600, 'label': '1 hour'},
                     {'value': 14400, 'label': '4 hours'},
                     {'value': 28800, 'label': '8 hours'},
                     {'value': 86400, 'label': '1 day'},
                     {'value': 604800, 'label': '1 week'},
                 ]},
                {'id': 'development_mode', 'label': 'Development mode', 'type': 'toggle',
                 'help': 'Temporarily bypass the cache while you work. Auto-expires after 3 hours.'},
                {'id': 'always_online', 'label': 'Always Online', 'type': 'toggle',
                 'help': 'Serve a cached copy of your site if your origin is unreachable.'},
            ],
        },
        {
            'key': 'security',
            'label': 'Security',
            'settings': [
                {'id': 'security_level', 'label': 'Security level', 'type': 'select',
                 'options': [
                     {'value': 'off', 'label': 'Off'},
                     {'value': 'essentially_off', 'label': 'Essentially off'},
                     {'value': 'low', 'label': 'Low'},
                     {'value': 'medium', 'label': 'Medium'},
                     {'value': 'high', 'label': 'High'},
                     {'value': 'under_attack', 'label': "I'm under attack"},
                 ]},
                {'id': 'browser_check', 'label': 'Browser integrity check', 'type': 'toggle',
                 'help': 'Block requests from common malicious bots and crawlers.'},
                {'id': 'challenge_ttl', 'label': 'Challenge passage', 'type': 'select',
                 'help': 'How long a visitor stays verified after passing a challenge.',
                 'options': [
                     {'value': 300, 'label': '5 minutes'},
                     {'value': 900, 'label': '15 minutes'},
                     {'value': 1800, 'label': '30 minutes'},
                     {'value': 3600, 'label': '1 hour'},
                     {'value': 7200, 'label': '2 hours'},
                     {'value': 10800, 'label': '3 hours'},
                     {'value': 14400, 'label': '4 hours'},
                     {'value': 28800, 'label': '8 hours'},
                     {'value': 86400, 'label': '1 day'},
                 ]},
            ],
        },
    ]

    # One-click hardening (plan §Phase 1 Actions): Full (strict), Always HTTPS,
    # HSTS (6 months), TLS 1.2 floor + 1.3, Brotli, HTTP/3, 4h browser cache.
    RECOMMENDED_PRESET = [
        ('ssl', 'strict'),
        ('always_use_https', 'on'),
        ('automatic_https_rewrites', 'on'),
        ('min_tls_version', '1.2'),
        ('tls_1_3', 'on'),
        ('brotli', 'on'),
        ('http3', 'on'),
        ('browser_cache_ttl', 14400),
        ('security_header', {'strict_transport_security': {
            'enabled': True, 'max_age': 15552000,
            'include_subdomains': True, 'preload': False, 'nosniff': True}}),
    ]

    @staticmethod
    def _zone_and_client(zone_id):
        """Resolve ``(zone, CloudflareClient)`` for a ServerKit DNS zone id, or raise
        :class:`CloudflareError` with a user-facing reason. Credential resolution
        reuses the canonical resolver so the connection store is the single source
        of truth."""
        from app.services.dns_zone_service import DNSZoneService
        from app.services.dns import CloudflareClient

        zone = DNSZoneService.get_zone(zone_id)
        if not zone:
            raise CloudflareError('Zone not found')
        if (zone.provider or '').lower() != 'cloudflare':
            raise CloudflareError('This zone is not managed by Cloudflare')
        credential = DNSZoneService._resolve_credential(zone)
        if not credential:
            raise CloudflareError('No connected Cloudflare credential resolves for this zone')
        if not zone.provider_zone_id:
            raise CloudflareError("Cloudflare hasn't been matched to this domain yet — "
                                  'open the DNS zone once to link it, then retry')
        return zone, CloudflareClient(credential)

    @staticmethod
    def _zone_dict(zone):
        return {'id': zone.id, 'domain': zone.domain,
                'provider_zone_id': zone.provider_zone_id}

    @classmethod
    def get_settings(cls, zone_id):
        """Live zone settings, indexed by id, plus the UI grouping metadata."""
        zone, client = cls._zone_and_client(zone_id)
        res = client.get_zone_settings(zone.provider_zone_id)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to load zone settings')}
        by_id = {s.get('id'): s for s in (res.get('result') or []) if isinstance(s, dict)}
        return {'success': True, 'zone': cls._zone_dict(zone),
                'groups': cls.SETTING_GROUPS, 'settings': by_id}

    @classmethod
    def get_setting(cls, zone_id, setting_id):
        zone, client = cls._zone_and_client(zone_id)
        res = client.get_zone_setting(zone.provider_zone_id, setting_id)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to load setting')}
        return {'success': True, 'setting': res.get('result')}

    @classmethod
    def update_setting(cls, zone_id, setting_id, value):
        zone, client = cls._zone_and_client(zone_id)
        res = client.update_zone_setting(zone.provider_zone_id, setting_id, value)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Update failed')}
        return {'success': True, 'setting': res.get('result')}

    @classmethod
    def apply_recommended(cls, zone_id):
        """Apply the recommended hardening preset, returning a per-setting report so
        the UI can show which toggles the plan allowed and which it gated."""
        zone, client = cls._zone_and_client(zone_id)
        results = []
        for setting_id, value in cls.RECOMMENDED_PRESET:
            res = client.update_zone_setting(zone.provider_zone_id, setting_id, value)
            results.append({'setting': setting_id,
                            'success': bool(res.get('success')),
                            'error': None if res.get('success') else res.get('error')})
        applied = sum(1 for r in results if r['success'])
        return {'success': applied > 0, 'applied': applied,
                'total': len(results), 'results': results}

    # Free/Pro plans can purge everything or up to 30 individual files per request;
    # hosts/prefixes/tags are Enterprise-only (Cloudflare returns a plan error,
    # which we surface verbatim).
    MAX_PURGE_FILES = 30

    @classmethod
    def purge_cache(cls, zone_id, *, everything=False, files=None, hosts=None,
                    prefixes=None, tags=None):
        """Purge the zone's Cloudflare cache. Either ``everything`` or one/more of
        ``files``/``hosts``/``prefixes``/``tags``. Raises :class:`CloudflareError`
        when nothing was requested (a caller error)."""
        zone, client = cls._zone_and_client(zone_id)

        if everything:
            payload = {'purge_everything': True}
        else:
            payload = {}
            clean = [f.strip() for f in (files or []) if f and f.strip()]
            if clean:
                payload['files'] = clean[:cls.MAX_PURGE_FILES]
            for key, val in (('hosts', hosts), ('prefixes', prefixes), ('tags', tags)):
                items = [v.strip() for v in (val or []) if v and v.strip()]
                if items:
                    payload[key] = items
            if not payload:
                raise CloudflareError('Nothing to purge — choose "everything" or '
                                      'provide files, hosts, prefixes, or tags')

        res = client.purge_cache(zone.provider_zone_id, payload)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Cache purge failed')}
        return {'success': True, 'purged': payload}

    # ── WAF custom rules ─────────────────────────────────────────────────────

    WAF_PHASE = 'http_request_firewall_custom'
    # Actions ServerKit lets you set on a custom rule (a safe subset of
    # Cloudflare's). Terminal actions only — no "skip", which can disable WAF.
    WAF_ACTIONS = {'block', 'managed_challenge', 'js_challenge', 'challenge', 'log'}

    # One-click rule templates (plan §Phase 3). ``params`` are collected from the
    # admin and validated before being interpolated into a Cloudflare expression.
    WAF_PRESETS = [
        {
            'key': 'lock_wp_admin',
            'label': 'Lock WordPress admin to an IP',
            'description': 'Block /wp-admin and /wp-login.php for everyone except a '
                           'trusted IP address (e.g. your office or home).',
            'action': 'block',
            'params': [{'key': 'ip', 'label': 'Allowed IP address',
                        'placeholder': 'e.g. 203.0.113.7'}],
        },
        {
            'key': 'block_exploit_paths',
            'label': 'Block common exploit paths',
            'description': 'Block requests probing for /xmlrpc.php, dotfiles like '
                           '/.env and /.git, and exposed config files.',
            'action': 'block',
            'params': [],
        },
        {
            'key': 'challenge_bad_bots',
            'label': 'Challenge suspicious bots',
            'description': 'Show a managed challenge to traffic with a low bot score. '
                           'Requires Cloudflare Bot Management on some plans.',
            'action': 'managed_challenge',
            'params': [],
        },
    ]

    @classmethod
    def _validate_action(cls, action):
        if action not in cls.WAF_ACTIONS:
            raise CloudflareError(
                f'Unsupported action "{action}". Use one of: {", ".join(sorted(cls.WAF_ACTIONS))}')

    @classmethod
    def _build_preset_rule(cls, key, params):
        """Turn a preset key + admin-supplied params into a concrete rule dict.
        Validates any user input that lands inside a Cloudflare expression (the IP)
        so a preset can't be used to inject expression syntax."""
        import ipaddress
        if key == 'lock_wp_admin':
            ip = (params.get('ip') or '').strip()
            try:
                ipaddress.ip_address(ip)
            except ValueError:
                raise CloudflareError('A valid IP address is required for this rule')
            return {
                'description': 'ServerKit: lock WordPress admin to a trusted IP',
                'expression': ('(http.request.uri.path contains "/wp-admin" or '
                               'http.request.uri.path contains "/wp-login.php") '
                               f'and ip.src ne {ip}'),
                'action': 'block',
            }
        if key == 'block_exploit_paths':
            return {
                'description': 'ServerKit: block common exploit paths',
                'expression': ('(http.request.uri.path contains "/xmlrpc.php") or '
                               '(http.request.uri.path contains "/.env") or '
                               '(http.request.uri.path contains "/.git/") or '
                               '(http.request.uri.path contains "/wp-config.php")'),
                'action': 'block',
            }
        if key == 'challenge_bad_bots':
            return {
                'description': 'ServerKit: challenge suspicious bots',
                'expression': '(cf.bot_management.score lt 30)',
                'action': 'managed_challenge',
            }
        raise CloudflareError(f'Unknown WAF preset: {key}')

    @classmethod
    def _find_custom_ruleset(cls, client, provider_zone_id):
        """Return ``(ruleset_dict, listing_error)``. ``ruleset_dict`` is the zone's
        custom-firewall entry-point ruleset or ``None`` when it doesn't exist yet;
        ``listing_error`` is set only when the list call itself failed (auth/scope)."""
        listing = client.list_rulesets(provider_zone_id)
        if not listing.get('success'):
            return None, listing.get('error', 'Failed to list rulesets')
        custom = next((rs for rs in (listing.get('result') or [])
                       if rs.get('phase') == cls.WAF_PHASE and rs.get('kind') == 'zone'), None)
        return custom, None

    @staticmethod
    def _rule_dict(r):
        return {'id': r.get('id'), 'description': r.get('description'),
                'expression': r.get('expression'), 'action': r.get('action'),
                'enabled': r.get('enabled', True), 'ref': r.get('ref')}

    @classmethod
    def list_waf_rules(cls, zone_id):
        """Custom firewall rules for the zone, plus the preset catalog. A zone with
        no custom ruleset yet returns an empty list (not an error)."""
        zone, client = cls._zone_and_client(zone_id)
        custom, err = cls._find_custom_ruleset(client, zone.provider_zone_id)
        if err:
            return {'success': False, 'error': err}
        if not custom:
            return {'success': True, 'ruleset_id': None, 'rules': [],
                    'presets': cls.WAF_PRESETS}
        detail = client.get_ruleset(zone.provider_zone_id, custom['id'])
        if not detail.get('success'):
            return {'success': False, 'error': detail.get('error', 'Failed to load ruleset')}
        result = detail.get('result') or {}
        rules = [cls._rule_dict(r) for r in (result.get('rules') or [])]
        return {'success': True, 'ruleset_id': result.get('id'),
                'rules': rules, 'presets': cls.WAF_PRESETS}

    @classmethod
    def add_waf_rule(cls, zone_id, *, description, expression, action, enabled=True):
        """Append a custom firewall rule, creating the zone's custom ruleset on first
        use. Returns the created rule's ruleset on success."""
        cls._validate_action(action)
        if not (expression or '').strip():
            raise CloudflareError('A rule expression is required')
        zone, client = cls._zone_and_client(zone_id)
        rule = {'description': description or 'ServerKit rule',
                'expression': expression, 'action': action, 'enabled': bool(enabled)}

        custom, err = cls._find_custom_ruleset(client, zone.provider_zone_id)
        if err:
            return {'success': False, 'error': err}
        if custom:
            res = client.add_ruleset_rule(zone.provider_zone_id, custom['id'], rule)
        else:
            res = client.create_phase_ruleset(zone.provider_zone_id, cls.WAF_PHASE, [rule])
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to add rule')}
        return {'success': True, 'result': res.get('result')}

    @classmethod
    def apply_waf_preset(cls, zone_id, preset_key, params=None):
        rule = cls._build_preset_rule(preset_key, params or {})
        return cls.add_waf_rule(zone_id, description=rule['description'],
                                expression=rule['expression'], action=rule['action'])

    @classmethod
    def update_waf_rule(cls, zone_id, ruleset_id, rule_id, fields):
        """Patch a custom rule. Only known fields are forwarded; an ``action``, if
        present, is validated."""
        zone, client = cls._zone_and_client(zone_id)
        rule = {}
        for key in ('description', 'expression', 'action', 'enabled'):
            if key in fields:
                rule[key] = fields[key]
        if 'action' in rule:
            cls._validate_action(rule['action'])
        if not rule:
            raise CloudflareError('No updatable fields provided')
        res = client.update_ruleset_rule(zone.provider_zone_id, ruleset_id, rule_id, rule)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to update rule')}
        return {'success': True, 'result': res.get('result')}

    @classmethod
    def delete_waf_rule(cls, zone_id, ruleset_id, rule_id):
        zone, client = cls._zone_and_client(zone_id)
        res = client.delete_ruleset_rule(zone.provider_zone_id, ruleset_id, rule_id)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to delete rule')}
        return {'success': True}

    # ── Workers (edge hosting) ───────────────────────────────────────────────
    # Workers are account-scoped; the owning account is read from the zone, so the
    # whole feature reuses the same Cloudflare connection the DNS zone already has.

    WORKER_NAME_RE = re.compile(r'^[a-z0-9][a-z0-9_-]{0,62}$')
    DEFAULT_COMPAT_DATE = '2025-01-01'

    @classmethod
    def _account_id(cls, zone, client):
        """The Cloudflare account that owns ``zone`` (for account-scoped resources)."""
        res = client.get_zone_account_id(zone.provider_zone_id)
        if not res.get('success'):
            raise CloudflareError(
                res.get('error') or 'Could not resolve the Cloudflare account for this zone')
        acct = ((res.get('result') or {}).get('account') or {}).get('id')
        if not acct:
            raise CloudflareError('Cloudflare did not return an account for this zone')
        return acct

    @classmethod
    def list_workers(cls, zone_id):
        """Live Worker scripts in the zone's account (flagged when ServerKit manages
        them), plus the zone's Worker routes."""
        from app.models.cloudflare_worker import CloudflareWorker
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        res = client.list_worker_scripts(account_id)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to list workers')}
        managed = {w.name: w for w in
                   CloudflareWorker.query.filter_by(account_id=account_id).all()}
        scripts = []
        for s in (res.get('result') or []):
            name = s.get('id')   # Cloudflare returns the script name in `id`
            rec = managed.get(name)
            scripts.append({'name': name, 'created_on': s.get('created_on'),
                            'modified_on': s.get('modified_on'),
                            'managed': rec is not None,
                            'has_source': bool(rec and rec.source)})
        routes = []
        rres = client.list_worker_routes(zone.provider_zone_id)
        if rres.get('success'):
            routes = [{'id': r.get('id'), 'pattern': r.get('pattern'), 'script': r.get('script')}
                      for r in (rres.get('result') or [])]
        return {'success': True, 'account_id': account_id, 'workers': scripts, 'routes': routes}

    @classmethod
    def deploy_worker(cls, zone_id, *, name, code, compatibility_date=None, route_pattern=None):
        """Upload a module Worker, record the source locally, and optionally attach a
        route in this zone."""
        from app import db
        from app.models.cloudflare_worker import CloudflareWorker

        name = (name or '').strip().lower()
        if not cls.WORKER_NAME_RE.match(name):
            raise CloudflareError('Worker name must be 1–63 chars: lowercase letters, '
                                  'digits, hyphens or underscores, starting alphanumeric')
        if not (code or '').strip():
            raise CloudflareError('Worker code is required')
        compat = compatibility_date or cls.DEFAULT_COMPAT_DATE

        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        res = client.upload_worker_module(account_id, name, code, compat)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Worker upload failed')}

        rec = CloudflareWorker.query.filter_by(account_id=account_id, name=name).first()
        if not rec:
            rec = CloudflareWorker(account_id=account_id, name=name,
                                   dns_provider_config_id=zone.dns_provider_config_id)
            db.session.add(rec)
        rec.source = code
        rec.compatibility_date = compat
        db.session.commit()

        route = None
        if route_pattern and route_pattern.strip():
            rr = client.add_worker_route(zone.provider_zone_id, route_pattern.strip(), name)
            route = {'success': bool(rr.get('success')),
                     'error': None if rr.get('success') else rr.get('error')}
        return {'success': True, 'worker': rec.to_dict(), 'route': route}

    @classmethod
    def delete_worker(cls, zone_id, name):
        from app import db
        from app.models.cloudflare_worker import CloudflareWorker
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        res = client.delete_worker_script(account_id, name)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to delete worker')}
        rec = CloudflareWorker.query.filter_by(account_id=account_id, name=name).first()
        if rec:
            db.session.delete(rec)
            db.session.commit()
        return {'success': True}

    @classmethod
    def add_worker_route(cls, zone_id, pattern, script):
        if not (pattern or '').strip() or not (script or '').strip():
            raise CloudflareError('Both a route pattern and a worker name are required')
        zone, client = cls._zone_and_client(zone_id)
        res = client.add_worker_route(zone.provider_zone_id, pattern.strip(), script.strip())
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to add route')}
        return {'success': True, 'result': res.get('result')}

    @classmethod
    def delete_worker_route(cls, zone_id, route_id):
        zone, client = cls._zone_and_client(zone_id)
        res = client.delete_worker_route(zone.provider_zone_id, route_id)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to delete route')}
        return {'success': True}

    # ── Tunnels (cloudflared) ────────────────────────────────────────────────
    # Cloudflare Tunnels expose a local/private service through Cloudflare's edge
    # without a public IP. Distinct from ServerKit's WireGuard remote-access
    # tunnels. Account-scoped; account resolved from the zone.

    @staticmethod
    def _install_command(token):
        """The one-liner an operator runs on the target host to attach a connector."""
        if not token:
            return None
        return f'cloudflared service install {token}'

    @classmethod
    def list_tunnels(cls, zone_id):
        from app.models.cloudflare_tunnel import CloudflareTunnel
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        res = client.list_tunnels(account_id)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to list tunnels')}
        managed = {t.tunnel_id for t in
                   CloudflareTunnel.query.filter_by(account_id=account_id).all()}
        tunnels = [{'id': t.get('id'), 'name': t.get('name'), 'status': t.get('status'),
                    'created_at': t.get('created_at'),
                    'connections': len(t.get('connections') or []),
                    'managed': t.get('id') in managed}
                   for t in (res.get('result') or [])]
        return {'success': True, 'account_id': account_id, 'tunnels': tunnels}

    @classmethod
    def create_tunnel(cls, zone_id, name):
        """Create a tunnel and return the connector token + install command (the
        token is revealed once here and stored encrypted for later)."""
        from app import db
        from app.models.cloudflare_tunnel import CloudflareTunnel
        from app.utils.crypto import encrypt_secret

        name = (name or '').strip()
        if not name:
            raise CloudflareError('A tunnel name is required')
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        res = client.create_tunnel(account_id, name)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to create tunnel')}
        result = res.get('result') or {}
        tunnel_id = result.get('id')
        token = result.get('token')
        if not token and tunnel_id:
            tres = client.get_tunnel_token(account_id, tunnel_id)
            token = tres.get('result') if tres.get('success') else None

        rec = CloudflareTunnel(
            tunnel_id=tunnel_id, name=name, account_id=account_id,
            dns_provider_config_id=zone.dns_provider_config_id,
            token_encrypted=encrypt_secret(token) if token else None)
        db.session.add(rec)
        db.session.commit()
        return {'success': True, 'tunnel': rec.to_dict(),
                'token': token, 'install': cls._install_command(token)}

    @classmethod
    def get_tunnel_install(cls, zone_id, tunnel_id):
        """Re-fetch the connector token + install command for a tunnel."""
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        tres = client.get_tunnel_token(account_id, tunnel_id)
        if not tres.get('success'):
            return {'success': False, 'error': tres.get('error', 'Failed to fetch token')}
        token = tres.get('result')
        return {'success': True, 'token': token, 'install': cls._install_command(token)}

    @classmethod
    def delete_tunnel(cls, zone_id, tunnel_id):
        from app import db
        from app.models.cloudflare_tunnel import CloudflareTunnel
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        res = client.delete_tunnel(account_id, tunnel_id)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to delete tunnel')}
        rec = CloudflareTunnel.query.filter_by(account_id=account_id, tunnel_id=tunnel_id).first()
        if rec:
            db.session.delete(rec)
            db.session.commit()
        return {'success': True}

    @classmethod
    def get_tunnel_hostnames(cls, zone_id, tunnel_id):
        """The public-hostname ingress rules configured on a tunnel."""
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        res = client.get_tunnel_configuration(account_id, tunnel_id)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to load tunnel config')}
        ingress = ((res.get('result') or {}).get('config') or {}).get('ingress') or []
        hostnames = [{'hostname': r.get('hostname'), 'service': r.get('service')}
                     for r in ingress if r.get('hostname')]
        return {'success': True, 'hostnames': hostnames}

    @classmethod
    def add_tunnel_hostname(cls, zone_id, tunnel_id, hostname, service):
        """Route a public hostname to a local service through the tunnel, and
        best-effort create the proxied CNAME that points it at the tunnel."""
        hostname = (hostname or '').strip().lower().rstrip('.')
        service = (service or '').strip()
        if not hostname or not service:
            raise CloudflareError('Both a hostname and a service '
                                  '(e.g. http://localhost:8080) are required')
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)

        cur = client.get_tunnel_configuration(account_id, tunnel_id)
        config = ((cur.get('result') or {}).get('config') or {}) if cur.get('success') else {}
        # Keep only real hostname rules, replace/insert this one, re-add catch-all.
        ingress = [r for r in (config.get('ingress') or [])
                   if r.get('hostname') and r.get('hostname') != hostname]
        ingress.append({'hostname': hostname, 'service': service})
        ingress.append({'service': 'http_status:404'})
        config['ingress'] = ingress

        res = client.put_tunnel_configuration(account_id, tunnel_id, config)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to set tunnel route')}
        dns = cls._ensure_tunnel_cname(zone, client, hostname, tunnel_id)
        return {'success': True, 'dns': dns}

    @classmethod
    def remove_tunnel_hostname(cls, zone_id, tunnel_id, hostname):
        hostname = (hostname or '').strip().lower().rstrip('.')
        if not hostname:
            raise CloudflareError('A hostname is required')
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        cur = client.get_tunnel_configuration(account_id, tunnel_id)
        config = ((cur.get('result') or {}).get('config') or {}) if cur.get('success') else {}
        ingress = [r for r in (config.get('ingress') or [])
                   if r.get('hostname') and r.get('hostname') != hostname]
        ingress.append({'service': 'http_status:404'})
        config['ingress'] = ingress
        res = client.put_tunnel_configuration(account_id, tunnel_id, config)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to remove tunnel route')}
        return {'success': True}

    @staticmethod
    def _ensure_tunnel_cname(zone, client, hostname, tunnel_id):
        """Best-effort: upsert the proxied CNAME ``hostname → <id>.cfargotunnel.com``
        that publishes the tunnel. Reported, never fatal — the route is already set."""
        try:
            from app.services.dns.base import DnsRecordSpec
            from app.services.dns_ownership_service import DnsOwnershipService
            target = f'{tunnel_id}.cfargotunnel.com'
            spec = DnsRecordSpec(record_type='CNAME', name=hostname, content=target,
                                 ttl=1, proxied=True)
            res = DnsOwnershipService.guarded_upsert(
                client, provider='cloudflare', provider_zone_id=zone.provider_zone_id,
                spec=spec, source='cf-tunnel', config_id=zone.dns_provider_config_id,
                allow_foreign=True)
            return {'created': bool(res.get('success')),
                    'error': None if res.get('success') else res.get('error')}
        except Exception as e:
            return {'created': False, 'error': str(e)}

    # ── Developer platform: R2 / KV / D1 ─────────────────────────────────────
    # Account-scoped storage. Management only — listing the inventory and
    # creating/deleting resources. R2 is S3-compatible, so a bucket made here can
    # later back ServerKit backups via the existing S3 storage backend (a separate
    # follow-up that mints scoped R2 access keys).

    R2_BUCKET_RE = re.compile(r'^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$')

    @classmethod
    def list_storage(cls, zone_id):
        """Combined developer-platform inventory. Each product is fetched
        independently so a missing token scope degrades to a per-product error
        rather than failing the whole tab."""
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        out = {'success': True, 'account_id': account_id,
               'r2': [], 'kv': [], 'd1': [], 'errors': {}}

        r2 = client.list_r2_buckets(account_id)
        if r2.get('success'):
            out['r2'] = [{'name': b.get('name'), 'creation_date': b.get('creation_date')}
                         for b in ((r2.get('result') or {}).get('buckets') or [])]
        else:
            out['errors']['r2'] = r2.get('error')

        kv = client.list_kv_namespaces(account_id)
        if kv.get('success'):
            out['kv'] = [{'id': n.get('id'), 'title': n.get('title')}
                         for n in (kv.get('result') or [])]
        else:
            out['errors']['kv'] = kv.get('error')

        d1 = client.list_d1_databases(account_id)
        if d1.get('success'):
            out['d1'] = [{'uuid': d.get('uuid'), 'name': d.get('name')}
                         for d in (d1.get('result') or [])]
        else:
            out['errors']['d1'] = d1.get('error')
        return out

    @classmethod
    def create_r2_bucket(cls, zone_id, name):
        name = (name or '').strip().lower()
        if not cls.R2_BUCKET_RE.match(name):
            raise CloudflareError('Bucket name must be 3–63 chars: lowercase letters, '
                                  'digits and hyphens, starting and ending alphanumeric')
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        res = client.create_r2_bucket(account_id, name)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to create bucket')}
        return {'success': True, 'bucket': name}

    @classmethod
    def delete_r2_bucket(cls, zone_id, name):
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        res = client.delete_r2_bucket(account_id, name)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to delete bucket')}
        return {'success': True}

    @classmethod
    def create_kv_namespace(cls, zone_id, title):
        title = (title or '').strip()
        if not title:
            raise CloudflareError('A namespace title is required')
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        res = client.create_kv_namespace(account_id, title)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to create namespace')}
        return {'success': True, 'namespace': res.get('result')}

    @classmethod
    def delete_kv_namespace(cls, zone_id, namespace_id):
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        res = client.delete_kv_namespace(account_id, namespace_id)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to delete namespace')}
        return {'success': True}

    @classmethod
    def create_d1_database(cls, zone_id, name):
        name = (name or '').strip()
        if not name:
            raise CloudflareError('A database name is required')
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        res = client.create_d1_database(account_id, name)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to create database')}
        return {'success': True, 'database': res.get('result')}

    @classmethod
    def delete_d1_database(cls, zone_id, database_id):
        zone, client = cls._zone_and_client(zone_id)
        account_id = cls._account_id(zone, client)
        res = client.delete_d1_database(account_id, database_id)
        if not res.get('success'):
            return {'success': False, 'error': res.get('error', 'Failed to delete database')}
        return {'success': True}
