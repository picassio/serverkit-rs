import os

def replace_in_file(path, old, new):
    if not os.path.exists(path):
        print(f'SKIP (not found): {path}')
        return False
    with open(path, 'r') as f:
        content = f.read()
    if old not in content:
        print(f'PATTERN NOT FOUND: {path}')
        return False
    content = content.replace(old, new)
    with open(path, 'w') as f:
        f.write(content)
    print(f'Patched: {path}')
    return True

# 1. site_domain_service.py - add panel_origin helper
replace_in_file(
    '/opt/serverkit/backend/app/services/site_domain_service.py',
    """    @classmethod
    def server_ip(cls):
        \"\"\"Public IP that wildcard/custom A-records should point at (Phase 3).\"\"\"
        return SystemSettings.get('server_public_ip') or current_app.config.get('SERVER_PUBLIC_IP') or None

    @classmethod
    def https_enabled(cls):""",
    """    @classmethod
    def server_ip(cls):
        \"\"\"Public IP that wildcard/custom A-records should point at (Phase 3).\"\"\"
        return SystemSettings.get('server_public_ip') or current_app.config.get('SERVER_PUBLIC_IP') or None

    @classmethod
    def panel_origin(cls):
        \"\"\"Canonical public origin of the ServerKit panel, or None when no
        canonical domain is configured.

        Uses the persisted canonical_domain / canonical_https_enabled settings.
        Falls back to PUBLIC_URL / SERVERKIT_PUBLIC_URL env vars, then to the
        sites base domain. Returns None if nothing usable is configured.
        \"\"\"
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
    def https_enabled(cls):"""
)

# 2. git_service.py - Gitea URL
replace_in_file(
    '/opt/serverkit/backend/app/services/git_service.py',
    """        return {
            'installed': True,
            'running': running,
            'http_port': app.port or config.get('http_port'),
            'ssh_port': config.get('ssh_port'),
            'url_path': '/gitea',  # Slug-based URL path
            'url': f\"http://localhost:{app.port}\" if app.port else None,  # Legacy port-based URL
            'app_id': app.id,
            'version': config.get('version', '1.21')
        }""",
    """        # Prefer the panel's canonical origin so the link works through a domain
        # and Cloudflare; fall back to the local port only when no domain is set.
        from app.services.site_domain_service import SiteDomainService
        panel_origin = SiteDomainService.panel_origin()
        if panel_origin:
            public_url = f\"{panel_origin}/gitea\"
        elif app.port:
            public_url = f\"http://localhost:{app.port}\"
        else:
            public_url = None

        return {
            'installed': True,
            'running': running,
            'http_port': app.port or config.get('http_port'),
            'ssh_port': config.get('ssh_port'),
            'url_path': '/gitea',
            'url': public_url,
            'app_id': app.id,
            'version': config.get('version', '1.21')
        }"""
)

# 3. wordpress_service.py - standalone WordPress status URL
replace_in_file(
    '/opt/serverkit/backend/app/services/wordpress_service.py',
    """        return {
            'installed': True,
            'running': running,
            'http_port': app.port or config.get('http_port'),
            'url_path': '/wordpress',
            'url': f\"http://localhost:{app.port}\" if app.port else None,
            'app_id': app.id,
            'version': config.get('version', '6.4')
        }""",
    """        # Prefer the panel's canonical origin so the link works through a domain
        # and Cloudflare; fall back to the local port only when no domain is set.
        from app.services.site_domain_service import SiteDomainService
        panel_origin = SiteDomainService.panel_origin()
        if panel_origin:
            public_url = f\"{panel_origin}/wordpress\"
        elif app.port:
            public_url = f\"http://localhost:{app.port}\"
        else:
            public_url = None

        return {
            'installed': True,
            'running': running,
            'http_port': app.port or config.get('http_port'),
            'url_path': '/wordpress',
            'url': public_url,
            'app_id': app.id,
            'version': config.get('version', '6.4')
        }"""
)

# 4. wordpress_service.py - _canonical_site_url fallback
replace_in_file(
    '/opt/serverkit/backend/app/services/wordpress_service.py',
    """    @classmethod
    def _canonical_site_url(cls, app) -> str:
        \"\"\"The URL WordPress serves under — its primary domain if one exists,
        else the legacy localhost:<port> address. Used to build correct
        search-replace pairs when cloning.\"\"\"
        from app.models.domain import Domain
        d = (Domain.query.filter_by(application_id=app.id, is_primary=True).first()
             or Domain.query.filter_by(application_id=app.id).first())
        if d:
            return f'http://{d.name}'
        if app.port:
            return f'http://localhost:{app.port}'
        return None""",
    """    @classmethod
    def _canonical_site_url(cls, app) -> str:
        \"\"\"The URL WordPress serves under — its primary domain if one exists,
        else the panel's canonical origin, else the legacy localhost:<port>
        address. Used to build correct search-replace pairs when cloning.\"\"\"
        from app.models.domain import Domain
        from app.services.site_domain_service import SiteDomainService
        d = (Domain.query.filter_by(application_id=app.id, is_primary=True).first()
             or Domain.query.filter_by(application_id=app.id).first())
        if d:
            scheme = 'https' if d.ssl_enabled else 'http'
            return f'{scheme}://{d.name}'
        panel_origin = SiteDomainService.panel_origin()
        if panel_origin:
            return panel_origin
        if app.port:
            return f'http://localhost:{app.port}'
        return None"""
)

# 5. roundcube_service.py - install signature + URL
replace_in_file(
    '/opt/serverkit/backend/app/services/roundcube_service.py',
    """    @classmethod
    def install(cls, imap_host: str = 'host.docker.internal',
                smtp_host: str = 'host.docker.internal') -> Dict:""",
    """    @classmethod
    def install(cls, imap_host: str = 'host.docker.internal',
                smtp_host: str = 'host.docker.internal',
                domain: str = None) -> Dict:"""
)

replace_in_file(
    '/opt/serverkit/backend/app/services/roundcube_service.py',
    """            return {
                'success': True,
                'message': 'Roundcube installed successfully',
                'port': cls.HOST_PORT,
                'url': f'http://localhost:{cls.HOST_PORT}',
            }""",
    """            # Build a public URL: prefer a supplied domain, then auto-generate
            # webmail.<panel_domain>, and only fall back to localhost:port.
            from app.services.site_domain_service import SiteDomainService
            public_url = None
            warning = None
            if domain:
                public_url = f'http://{domain}'
                proxy_res = cls.configure_nginx_proxy(domain)
                if not proxy_res.get('success'):
                    warning = proxy_res.get('error')
            else:
                panel_origin = SiteDomainService.panel_origin()
                panel_host = None
                if panel_origin:
                    from urllib.parse import urlparse
                    panel_host = urlparse(panel_origin).hostname
                if panel_host:
                    webmail_domain = f'webmail.{panel_host}'
                    public_url = f'http://{webmail_domain}'
                    proxy_res = cls.configure_nginx_proxy(webmail_domain)
                    if not proxy_res.get('success'):
                        warning = proxy_res.get('error')

            if not public_url:
                public_url = f'http://localhost:{cls.HOST_PORT}'

            result = {
                'success': True,
                'message': 'Roundcube installed successfully',
                'port': cls.HOST_PORT,
                'url': public_url,
            }
            if warning:
                result['warning'] = warning
            return result"""
)

# 6. email.py - pass domain to roundcube install
replace_in_file(
    '/opt/serverkit/backend/app/api/email.py',
    """    result = RoundcubeService.install(
        imap_host=data.get('imap_host', 'host.docker.internal'),
        smtp_host=data.get('smtp_host', 'host.docker.internal'),
    )""",
    """    result = RoundcubeService.install(
        imap_host=data.get('imap_host', 'host.docker.internal'),
        smtp_host=data.get('smtp_host', 'host.docker.internal'),
        domain=(data.get('domain') or '').strip() or None,
    )"""
)

# 7. python_service.py - Django ALLOWED_HOSTS
replace_in_file(
    '/opt/serverkit/backend/app/services/python_service.py',
    """            # Create .env file
            env_content = f'''DEBUG=False
SECRET_KEY={secrets.token_hex(32)}
ALLOWED_HOSTS=localhost,127.0.0.1
'''
            with open(os.path.join(app_path, '.env'), 'w') as f:
                f.write(env_content)""",
    """            # Build ALLOWED_HOSTS: local addresses plus the panel domain and its
            # subdomains so Django accepts requests behind nginx on a real domain.
            from app.services.site_domain_service import SiteDomainService
            allowed_hosts = ['localhost', '127.0.0.1']
            panel_origin = SiteDomainService.panel_origin()
            if panel_origin:
                from urllib.parse import urlparse
                panel_host = urlparse(panel_origin).hostname
                if panel_host:
                    allowed_hosts.append(panel_host)
                    # Permit subdomains of the panel domain (e.g. app.serverkit.example.com)
                    if '.' in panel_host:
                        allowed_hosts.append('.' + panel_host)
            allowed_hosts_str = ','.join(allowed_hosts)

            # Create .env file
            env_content = f'''DEBUG=False
SECRET_KEY={secrets.token_hex(32)}
ALLOWED_HOSTS={allowed_hosts_str}
'''
            with open(os.path.join(app_path, '.env'), 'w') as f:
                f.write(env_content)"""
)

# 8. environment_domain_service.py - fallback
replace_in_file(
    '/opt/serverkit/backend/app/services/environment_domain_service.py',
    """        if not production_domain:
            # Fallback for sites without a domain
            slug = cls.slugify(branch_name) if branch_name else env_type
            return f'{env_type}-{slug}.localhost'""",
    """        if not production_domain:
            # Fallback for sites without a domain: use the managed-sites base domain
            # or the panel domain instead of a useless .localhost address.
            from app.services.site_domain_service import SiteDomainService
            base = SiteDomainService.base_domain() or SiteDomainService.panel_origin()
            if base:
                from urllib.parse import urlparse
                base_host = urlparse(base).hostname or base
                slug = cls.slugify(branch_name) if branch_name else env_type
                return f'{env_type}-{slug}.{base_host}'
            # Last resort only when absolutely no domain is configured
            slug = cls.slugify(branch_name) if branch_name else env_type
            return f'{env_type}-{slug}.localhost'"""
)

print('All backend patches applied.')
