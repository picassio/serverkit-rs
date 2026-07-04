import os
import subprocess
import re
from typing import Dict, List, Optional
from pathlib import Path

from app.utils.system import ServiceControl, run_privileged, is_command_available


def _validate_domain(domain: str) -> bool:
    """Validate domain name to prevent nginx config injection."""
    return bool(re.match(r'^(?:[a-z0-9](?:[a-z0-9\-]{0,61}[a-z0-9])?\.)*[a-z0-9](?:[a-z0-9\-]{0,61}[a-z0-9])?$', domain, re.IGNORECASE))


def _validate_path(path: str) -> bool:
    """Validate filesystem path for nginx config."""
    # Block path traversal and special characters
    if '..' in path or '\n' in path or '\r' in path or ';' in path:
        return False
    return bool(re.match(r'^/[a-zA-Z0-9/_\-\.]+$', path))


class NginxService:
    """Service for Nginx configuration management."""

    # Default paths (can be overridden via environment)
    NGINX_CONF_DIR = os.environ.get('NGINX_CONF_DIR', '/etc/nginx')
    SITES_AVAILABLE = os.path.join(NGINX_CONF_DIR, 'sites-available')
    SITES_ENABLED = os.path.join(NGINX_CONF_DIR, 'sites-enabled')
    NGINX_BIN = os.environ.get('NGINX_BIN', '/usr/sbin/nginx')
    LOCATIONS_DIR = os.path.join(NGINX_CONF_DIR, 'serverkit-locations')
    # Per-site log directory. Must stay in sync with the access_log/error_log
    # paths hard-coded in the site templates below. site_access_log_path() is the
    # single source of truth consumers should use (e.g. the fail2ban jail layer
    # watches this exact file), so the path can never drift from what nginx writes.
    LOG_DIR = '/var/log/nginx'

    # Templates
    PHP_SITE_TEMPLATE = '''server {{
    listen 80;
    listen [::]:80;
    server_name {domains};

    root {root_path};
    index index.php index.html index.htm;

    access_log /var/log/nginx/{name}.access.log;
    error_log /var/log/nginx/{name}.error.log;

    location / {{
        try_files $uri $uri/ /index.php?$query_string;
    }}

    location ~ \\.php$ {{
        fastcgi_pass unix:/run/php/php{php_version}-fpm.sock;
        fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;
        include fastcgi_params;
        fastcgi_intercept_errors on;
        fastcgi_buffer_size 16k;
        fastcgi_buffers 4 16k;
    }}

    location ~ /\\.ht {{
        deny all;
    }}

    location = /favicon.ico {{
        log_not_found off;
        access_log off;
    }}

    location = /robots.txt {{
        log_not_found off;
        access_log off;
        allow all;
    }}

    location ~* \\.(css|gif|ico|jpeg|jpg|js|png|svg|woff|woff2)$ {{
        expires 1y;
        log_not_found off;
    }}
}}
'''

    PYTHON_SITE_TEMPLATE = '''server {{
    listen 80;
    listen [::]:80;
    server_name {domains};

    access_log /var/log/nginx/{name}.access.log;
    error_log /var/log/nginx/{name}.error.log;

    location / {{
        proxy_pass http://127.0.0.1:{port};
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
        proxy_read_timeout 86400;
    }}

    location /static {{
        alias {root_path}/static;
        expires 1y;
    }}
}}
'''

    STATIC_SITE_TEMPLATE = '''server {{
    listen 80;
    listen [::]:80;
    server_name {domains};

    root {root_path};
    index index.html index.htm;

    access_log /var/log/nginx/{name}.access.log;
    error_log /var/log/nginx/{name}.error.log;

    location / {{
        try_files $uri $uri/ =404;
    }}

    location ~* \\.(css|gif|ico|jpeg|jpg|js|png|svg|woff|woff2)$ {{
        expires 1y;
        log_not_found off;
    }}
}}
'''

    # Docker reverse proxy template (for containerized apps)
    DOCKER_SITE_TEMPLATE = '''server {{
    listen 80;
    listen [::]:80;
    server_name {domains};

    access_log /var/log/nginx/{name}.access.log;
    error_log /var/log/nginx/{name}.error.log;

    location / {{
        proxy_pass http://127.0.0.1:{port};
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
        proxy_read_timeout 86400;
        proxy_connect_timeout 60;
        proxy_send_timeout 60;
    }}
}}
'''

    # Reverse proxy to a service reached over a WireGuard tunnel (roadmap
    # #12). The upstream is a peer's WG IP:port; the private agent forwards
    # it to the real service. proxy_buffering off keeps media/streaming
    # (e.g. Jellyfin) responsive.
    REMOTE_UPSTREAM_TEMPLATE = '''server {{
    listen 80;
    listen [::]:80;
    server_name {domains};

    access_log /var/log/nginx/{name}.access.log;
    error_log /var/log/nginx/{name}.error.log;

    location / {{
        proxy_pass http://{upstream};
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
        proxy_buffering off;
        proxy_read_timeout 86400;
        proxy_connect_timeout 60;
        proxy_send_timeout 60;
    }}
}}
'''

    SSL_BLOCK = '''
    listen 443 ssl http2;
    listen [::]:443 ssl http2;

    ssl_certificate {ssl_cert};
    ssl_certificate_key {ssl_key};
    ssl_session_timeout 1d;
    ssl_session_cache shared:SSL:50m;
    ssl_session_tickets off;

    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;
    ssl_ecdh_curve X25519:secp384r1;

    # HSTS: 2 years, all subdomains, preload-eligible. Submit the domain at
    # https://hstspreload.org to get onto the browser preload list (this is what
    # protects first-time visitors from an initial-request MITM downgrade).
    add_header Strict-Transport-Security "max-age=63072000; includeSubDomains; preload" always;

    # Baseline response-header hardening. These are safe for reverse-proxied apps;
    # the CSP is deliberately permissive so it does not break managed apps while
    # still satisfying "a CSP is present" and blocking cross-origin framing.
    # Tighten the CSP per-app when the upstream can support a stricter policy.
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;
    add_header Content-Security-Policy "default-src 'self' 'unsafe-inline' 'unsafe-eval' data: blob: https:; frame-ancestors 'self'; upgrade-insecure-requests" always;
'''

    SSL_REDIRECT_TEMPLATE = '''server {{
    listen 80;
    listen [::]:80;
    server_name {domains};
    return 301 https://$server_name$request_uri;
}}
'''

    # Private URL routing templates
    PRIVATE_URL_CONFIG_NAME = 'serverkit-private-urls'
    GITEA_CONFIG_NAME = 'serverkit-gitea'
    WORDPRESS_CONFIG_NAME = 'serverkit-wordpress'

    # Gitea location block for /gitea path (included inside main server block)
    GITEA_LOCATION_TEMPLATE = '''# Gitea at /gitea path
location /gitea/ {{
    proxy_pass http://127.0.0.1:{port}/;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header X-Forwarded-Host $host;
    proxy_cache_bypass $http_upgrade;
    proxy_read_timeout 86400;
    proxy_connect_timeout 60;
    proxy_send_timeout 60;

    # Required for Gitea WebSocket connections
    proxy_buffering off;
    client_max_body_size 100M;
}}

# Handle /gitea without trailing slash
location = /gitea {{
    return 301 /gitea/;
}}
'''

    # WordPress location block for /wordpress path (included inside main server block)
    WORDPRESS_LOCATION_TEMPLATE = '''# WordPress at /wordpress path
location /wordpress/ {{
    proxy_pass http://127.0.0.1:{port}/;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header X-Forwarded-Host $host;
    proxy_cache_bypass $http_upgrade;
    proxy_read_timeout 300;
    proxy_connect_timeout 60;
    proxy_send_timeout 60;

    # WordPress file uploads
    client_max_body_size 256M;
}}

# Handle /wordpress without trailing slash
location = /wordpress {{
    return 301 /wordpress/;
}}
'''

    PRIVATE_URL_LOCATION_TEMPLATE = '''    # Private URL: /p/{slug}
    location /p/{slug}/ {{
        proxy_pass http://127.0.0.1:{port}/;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Private-URL {slug};
        proxy_cache_bypass $http_upgrade;
        proxy_read_timeout 86400;
    }}

    # Also handle without trailing slash
    location = /p/{slug} {{
        return 301 /p/{slug}/;
    }}
'''

    PRIVATE_URL_MAIN_CONFIG = '''# ServerKit Private URL Routes
# This file is auto-generated. Do not edit manually.
# Generated at: {timestamp}

{locations}

# Fallback for unknown slugs under /p/
location /p/ {{
    return 404;
}}
'''

    @classmethod
    def test_config(cls) -> Dict:
        """Test Nginx configuration syntax."""
        if not is_command_available('nginx'):
            return {'success': False, 'error': 'nginx is not installed'}

        try:
            result = run_privileged([cls.NGINX_BIN, '-t'], timeout=30)
            return {
                'success': result.returncode == 0,
                'message': result.stderr if result.returncode == 0 else result.stderr
            }
        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def reload(cls) -> Dict:
        """Reload Nginx configuration."""
        # Test config first
        test_result = cls.test_config()
        if not test_result['success']:
            return {'success': False, 'error': f"Config test failed: {test_result.get('message', test_result.get('error'))}"}

        try:
            result = ServiceControl.reload('nginx', timeout=30)
            return {
                'success': result.returncode == 0,
                'message': 'Nginx reloaded successfully' if result.returncode == 0 else result.stderr
            }
        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def restart(cls) -> Dict:
        """Restart Nginx service."""
        try:
            result = ServiceControl.restart('nginx', timeout=30)
            return {
                'success': result.returncode == 0,
                'message': 'Nginx restarted successfully' if result.returncode == 0 else result.stderr
            }
        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def get_status(cls) -> Dict:
        """Get Nginx service status."""
        try:
            result = run_privileged(['systemctl', 'status', 'nginx'], timeout=30)

            # Parse status
            is_running = 'active (running)' in result.stdout

            return {
                'running': is_running,
                'status': 'running' if is_running else 'stopped',
                'details': result.stdout
            }
        except Exception as e:
            return {'running': False, 'status': 'unknown', 'error': str(e)}

    @classmethod
    def list_sites(cls) -> List[Dict]:
        """List all configured sites."""
        sites = []

        if not os.path.exists(cls.SITES_AVAILABLE):
            return sites

        enabled_sites = set()
        if os.path.exists(cls.SITES_ENABLED):
            enabled_sites = {f for f in os.listdir(cls.SITES_ENABLED)}

        for filename in os.listdir(cls.SITES_AVAILABLE):
            if filename.startswith('.'):
                continue

            filepath = os.path.join(cls.SITES_AVAILABLE, filename)
            if os.path.isfile(filepath):
                config = cls._parse_site_config(filepath)
                sites.append({
                    'name': filename,
                    'enabled': filename in enabled_sites,
                    'domains': config.get('domains', []),
                    'root': config.get('root'),
                    'ssl': config.get('ssl', False)
                })

        return sites

    @classmethod
    def _parse_site_config(cls, filepath: str) -> Dict:
        """Parse basic info from a site config file."""
        config = {'domains': [], 'root': None, 'ssl': False}

        try:
            with open(filepath, 'r') as f:
                content = f.read()

            # Extract server_name
            match = re.search(r'server_name\s+([^;]+);', content)
            if match:
                domains = match.group(1).strip().split()
                config['domains'] = [d for d in domains if d != '_']

            # Extract root
            match = re.search(r'root\s+([^;]+);', content)
            if match:
                config['root'] = match.group(1).strip()

            # Check for SSL
            config['ssl'] = 'ssl_certificate' in content

        except Exception:
            pass

        return config

    @classmethod
    def create_site(cls, name: str, app_type: str, domains: List[str],
                    root_path: str, port: int = None, php_version: str = '8.2',
                    ssl_cert: str = None, ssl_key: str = None,
                    upstream: str = None) -> Dict:
        """Create a new site configuration.

        When ``ssl_cert``/``ssl_key`` are given the generated vhost serves HTTPS
        (443) from that cert with an HTTP->HTTPS redirect — used to point managed
        subdomains at the base-domain wildcard cert.
        """
        if not domains:
            return {'success': False, 'error': 'At least one domain is required'}

        # Validate all domains
        for domain in domains:
            if domain == '_':
                return {'success': False, 'error': 'Wildcard server_name "_" is reserved for the ServerKit panel'}
            if not _validate_domain(domain):
                return {'success': False, 'error': f'Invalid domain name: {domain}'}

        # Validate root_path if provided
        if root_path and not _validate_path(root_path):
            return {'success': False, 'error': f'Invalid root path: {root_path}'}

        domains_str = ' '.join(domains)

        # Select template based on app type
        if app_type in ['php', 'wordpress']:
            config = cls.PHP_SITE_TEMPLATE.format(
                name=name,
                domains=domains_str,
                root_path=root_path,
                php_version=php_version
            )
        elif app_type in ['flask', 'django', 'python']:
            if not port:
                return {'success': False, 'error': 'Port is required for Python apps'}
            config = cls.PYTHON_SITE_TEMPLATE.format(
                name=name,
                domains=domains_str,
                root_path=root_path,
                port=port
            )
        elif app_type == 'docker':
            if not port:
                return {'success': False, 'error': 'Port is required for Docker apps'}
            config = cls.DOCKER_SITE_TEMPLATE.format(
                name=name,
                domains=domains_str,
                port=port
            )
        elif app_type == 'remote':
            if not upstream:
                return {'success': False, 'error': 'upstream (host:port) is required for remote apps'}
            config = cls.REMOTE_UPSTREAM_TEMPLATE.format(
                name=name,
                domains=domains_str,
                upstream=upstream,
            )
        elif app_type == 'static':
            config = cls.STATIC_SITE_TEMPLATE.format(
                name=name,
                domains=domains_str,
                root_path=root_path
            )
        else:
            return {'success': False, 'error': f'Unknown app type: {app_type}'}

        if ssl_cert and ssl_key:
            config = cls._with_ssl(config, domains_str, ssl_cert, ssl_key)

        # Write config file
        config_path = os.path.join(cls.SITES_AVAILABLE, name)
        try:
            # Use sudo to write
            process = run_privileged(
                ['tee', config_path],
                input=config,
            )
            if process.returncode != 0:
                return {'success': False, 'error': process.stderr}

            return {'success': True, 'message': f'Site {name} created', 'path': config_path}
        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def _with_ssl(cls, http_config: str, domains: str, ssl_cert: str, ssl_key: str) -> str:
        """Turn an HTTP server block into HTTPS: swap the :80 listens for the TLS
        block (so the existing server serves 443) and prepend an HTTP->HTTPS
        redirect. Idempotent because create_site regenerates the whole file."""
        ssl_block = cls.SSL_BLOCK.format(ssl_cert=ssl_cert, ssl_key=ssl_key)
        https_config = http_config.replace(
            '    listen 80;\n    listen [::]:80;',
            ssl_block.strip('\n'),
            1,
        )
        redirect = cls.SSL_REDIRECT_TEMPLATE.format(domains=domains)
        return redirect + '\n' + https_config

    @classmethod
    def site_access_log_path(cls, name: str) -> str:
        """Path of the per-site nginx access log for site ``name`` — the same
        ``/var/log/nginx/{name}.access.log`` the site templates write. This is the
        canonical accessor so consumers (e.g. the fail2ban brute-force jail) never
        hard-code the path and it can't drift from the generated vhost. Always a
        POSIX path (these are Linux server logs), independent of the panel's OS."""
        return f'{cls.LOG_DIR}/{name}.access.log'

    @classmethod
    def site_error_log_path(cls, name: str) -> str:
        """Path of the per-site nginx error log for site ``name`` (companion to
        :meth:`site_access_log_path`)."""
        return f'{cls.LOG_DIR}/{name}.error.log'

    @classmethod
    def enable_site(cls, name: str) -> Dict:
        """Enable a site by creating symlink in sites-enabled."""
        available_path = os.path.join(cls.SITES_AVAILABLE, name)
        enabled_path = os.path.join(cls.SITES_ENABLED, name)

        if not os.path.exists(available_path):
            return {'success': False, 'error': f'Site {name} not found in sites-available'}

        try:
            result = run_privileged(['ln', '-sf', available_path, enabled_path])
            if result.returncode == 0:
                # Reload nginx
                reload_result = cls.reload()
                if reload_result['success']:
                    return {'success': True, 'message': f'Site {name} enabled'}
                return reload_result
            return {'success': False, 'error': result.stderr}
        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def disable_site(cls, name: str) -> Dict:
        """Disable a site by removing symlink from sites-enabled."""
        enabled_path = os.path.join(cls.SITES_ENABLED, name)

        try:
            result = run_privileged(['rm', '-f', enabled_path])
            if result.returncode == 0:
                reload_result = cls.reload()
                if reload_result['success']:
                    return {'success': True, 'message': f'Site {name} disabled'}
                return reload_result
            return {'success': False, 'error': result.stderr}
        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def delete_site(cls, name: str) -> Dict:
        """Delete a site configuration."""
        # First disable it
        cls.disable_site(name)

        available_path = os.path.join(cls.SITES_AVAILABLE, name)
        try:
            result = run_privileged(['rm', '-f', available_path])
            if result.returncode == 0:
                return {'success': True, 'message': f'Site {name} deleted'}
            return {'success': False, 'error': result.stderr}
        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def add_ssl_to_site(cls, name: str, cert_path: str, key_path: str) -> Dict:
        """Add SSL configuration to an existing site."""
        config_path = os.path.join(cls.SITES_AVAILABLE, name)

        if not os.path.exists(config_path):
            return {'success': False, 'error': f'Site {name} not found'}

        try:
            with open(config_path, 'r') as f:
                content = f.read()

            # Extract domains for redirect
            match = re.search(r'server_name\s+([^;]+);', content)
            domains_str = match.group(1).strip() if match else name

            # Add SSL block after listen 80 lines
            ssl_config = cls.SSL_BLOCK.format(ssl_cert=cert_path, ssl_key=key_path)

            # Add redirect server block
            redirect_block = cls.SSL_REDIRECT_TEMPLATE.format(domains=domains_str)

            # Modify existing config - add SSL listen and certs
            new_content = content.replace(
                'listen 80;',
                f'listen 80;\n{ssl_config}'
            )

            # Prepend redirect block
            final_content = redirect_block + '\n' + new_content

            # Write updated config
            process = run_privileged(
                ['tee', config_path],
                input=final_content,
            )

            if process.returncode == 0:
                return cls.reload()
            return {'success': False, 'error': process.stderr}

        except Exception as e:
            return {'success': False, 'error': str(e)}

    # ==================== DIAGNOSTICS ====================

    @classmethod
    def get_site_config(cls, name: str) -> Dict:
        """Get the content of a site configuration file.

        Args:
            name: The site name (config filename)

        Returns:
            Dict with exists, enabled, content, and path
        """
        config_path = os.path.join(cls.SITES_AVAILABLE, name)
        if not os.path.exists(config_path):
            return {'exists': False, 'error': 'Config file not found', 'path': config_path}

        try:
            with open(config_path, 'r') as f:
                content = f.read()

            enabled_path = os.path.join(cls.SITES_ENABLED, name)
            is_enabled = os.path.exists(enabled_path) or os.path.islink(enabled_path)

            # Parse some basic info from the config
            parsed = cls._parse_site_config(config_path)

            return {
                'exists': True,
                'enabled': is_enabled,
                'content': content,
                'path': config_path,
                'enabled_path': enabled_path,
                'domains': parsed.get('domains', []),
                'ssl': parsed.get('ssl', False)
            }
        except Exception as e:
            return {'exists': True, 'error': str(e), 'path': config_path}

    @classmethod
    def diagnose_site(cls, name: str, port: int = None) -> Dict:
        """Full diagnostic for a site configuration.

        Args:
            name: The site name
            port: Optional port to check accessibility

        Returns:
            Dict with comprehensive diagnostic information
        """
        diagnosis = {
            'site_name': name,
            'config': cls.get_site_config(name),
            'nginx_status': cls.get_status(),
            'config_test': cls.test_config()
        }

        # Check port accessibility if provided
        if port:
            from app.services.docker_service import DockerService
            diagnosis['port_check'] = DockerService.check_port_accessible(port)

        # Determine overall health
        config_ok = diagnosis['config'].get('exists', False)
        enabled_ok = diagnosis['config'].get('enabled', False)
        nginx_ok = diagnosis['nginx_status'].get('running', False)
        syntax_ok = diagnosis['config_test'].get('success', False)
        port_ok = diagnosis.get('port_check', {}).get('accessible', True)  # True if no port to check

        diagnosis['health'] = {
            'config_exists': config_ok,
            'config_enabled': enabled_ok,
            'nginx_running': nginx_ok,
            'syntax_valid': syntax_ok,
            'port_accessible': port_ok,
            'overall': all([config_ok, enabled_ok, nginx_ok, syntax_ok, port_ok])
        }

        # Generate recommendations
        recommendations = []
        if not config_ok:
            recommendations.append('Create Nginx site configuration')
        if config_ok and not enabled_ok:
            recommendations.append('Enable the site with NginxService.enable_site()')
        if not nginx_ok:
            recommendations.append('Start Nginx service')
        if not syntax_ok:
            recommendations.append(f"Fix Nginx config syntax: {diagnosis['config_test'].get('message', '')}")
        if port and not port_ok:
            recommendations.append(f'Ensure container is running and exposing port {port}')

        diagnosis['recommendations'] = recommendations

        return diagnosis

    @classmethod
    def check_site_routing(cls, name: str, domain: str, port: int) -> Dict:
        """Test the full routing chain for a site.

        Args:
            name: The site name
            domain: The domain to test
            port: The backend port

        Returns:
            Dict with routing test results
        """
        import urllib.request
        import urllib.error

        results = {
            'site_name': name,
            'domain': domain,
            'port': port,
            'tests': {}
        }

        # Test 1: Port accessibility
        from app.services.docker_service import DockerService
        results['tests']['port_accessible'] = DockerService.check_port_accessible(port)

        # Test 2: Direct backend request
        try:
            req = urllib.request.Request(f'http://127.0.0.1:{port}/', method='HEAD')
            req.add_header('User-Agent', 'ServerKit-Diagnostic/1.0')
            with urllib.request.urlopen(req, timeout=5) as response:
                results['tests']['backend_responds'] = {
                    'success': True,
                    'status_code': response.status
                }
        except urllib.error.HTTPError as e:
            results['tests']['backend_responds'] = {
                'success': True,  # HTTP error still means backend responded
                'status_code': e.code
            }
        except Exception as e:
            results['tests']['backend_responds'] = {
                'success': False,
                'error': str(e)
            }

        # Test 3: Config exists and enabled
        config = cls.get_site_config(name)
        results['tests']['config_status'] = {
            'exists': config.get('exists', False),
            'enabled': config.get('enabled', False)
        }

        return results

    # ==================== PRIVATE URL MANAGEMENT ====================

    @classmethod
    def update_private_url_config(cls, app, old_slug: str = None) -> Dict:
        """Update Nginx config for a private URL.

        This regenerates the entire private URLs config file since changes
        are infrequent and full regeneration is simpler.

        Args:
            app: The Application object with private_slug and port
            old_slug: Optional old slug being replaced (unused, kept for API compatibility)

        Returns:
            Dict with success status and message
        """
        return cls.regenerate_all_private_urls()

    @classmethod
    def remove_private_url_config(cls, slug: str) -> Dict:
        """Remove a private URL from Nginx config.

        This regenerates the entire private URLs config file.

        Args:
            slug: The slug being removed (unused, full regeneration happens)

        Returns:
            Dict with success status and message
        """
        return cls.regenerate_all_private_urls()

    @classmethod
    def regenerate_all_private_urls(cls) -> Dict:
        """Regenerate the entire private URLs config from database.

        Queries all applications with private URLs enabled and regenerates
        the Nginx configuration file with all location blocks.

        Returns:
            Dict with success status, message, and count of URLs configured
        """
        from datetime import datetime
        from app.models import Application

        try:
            # Query all apps with private URLs enabled
            apps = Application.query.filter(
                Application.private_url_enabled == True,
                Application.private_slug.isnot(None),
                Application.port.isnot(None)
            ).all()

            # Generate location blocks
            locations = []
            for app in apps:
                location = cls.PRIVATE_URL_LOCATION_TEMPLATE.format(
                    slug=app.private_slug,
                    port=app.port
                )
                locations.append(location)

            # Generate full config (location-only snippet, included by serverkit.conf)
            cls._ensure_locations_dir()
            if locations:
                config = cls.PRIVATE_URL_MAIN_CONFIG.format(
                    timestamp=datetime.utcnow().isoformat(),
                    locations='\n'.join(locations)
                )
            else:
                # No private URLs - create minimal fallback
                config = f'''# ServerKit Private URL Routes
# This file is auto-generated. Do not edit manually.
# Generated at: {datetime.utcnow().isoformat()}
# No private URLs configured

location /p/ {{
    return 404;
}}
'''

            # Write location snippet to serverkit-locations dir
            config_path = os.path.join(cls.LOCATIONS_DIR, f'{cls.PRIVATE_URL_CONFIG_NAME}.conf')
            process = run_privileged(['tee', config_path], input=config)
            if process.returncode != 0:
                return {'success': False, 'error': f'Failed to write config: {process.stderr}'}

            # Clean up legacy separate server block if it exists
            cls._remove_legacy_site(cls.PRIVATE_URL_CONFIG_NAME)

            # Reload Nginx
            reload_result = cls.reload()
            if not reload_result['success']:
                return reload_result

            return {
                'success': True,
                'message': f'Private URL config regenerated with {len(apps)} URLs',
                'url_count': len(apps),
                'config_path': config_path
            }

        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def get_private_url_config(cls) -> Dict:
        """Get the current private URL configuration.

        Returns:
            Dict with exists, enabled, content, and URL count
        """
        return cls.get_location_config(cls.PRIVATE_URL_CONFIG_NAME)

    # ==================== GITEA CONFIGURATION ====================

    @classmethod
    def get_location_config(cls, name: str) -> Dict:
        """Get a location snippet config from the serverkit-locations directory.

        Args:
            name: Config name (without .conf extension)

        Returns:
            Dict with exists, enabled, content, and path
        """
        config_path = os.path.join(cls.LOCATIONS_DIR, f'{name}.conf')
        config = {
            'exists': os.path.isfile(config_path),
            'enabled': os.path.isfile(config_path),  # If it exists, it's included
            'content': None,
            'path': config_path
        }

        if config['exists']:
            try:
                with open(config_path, 'r') as f:
                    config['content'] = f.read()
            except Exception:
                pass

        return config

    @classmethod
    def _ensure_locations_dir(cls):
        """Ensure the serverkit-locations directory exists."""
        run_privileged(['mkdir', '-p', cls.LOCATIONS_DIR])

    @classmethod
    def create_gitea_config(cls, port: int) -> Dict:
        """Create Nginx location config for Gitea at /gitea path.

        The config is a location-only snippet included inside the main
        serverkit.conf server block, preventing server_name conflicts.

        Args:
            port: The internal port Gitea is running on

        Returns:
            Dict with success status and message
        """
        try:
            cls._ensure_locations_dir()
            config = cls.GITEA_LOCATION_TEMPLATE.format(port=port)
            config_path = os.path.join(cls.LOCATIONS_DIR, f'{cls.GITEA_CONFIG_NAME}.conf')

            # Write location snippet (no separate server block, no sites-enabled symlink)
            process = run_privileged(['tee', config_path], input=config)
            if process.returncode != 0:
                return {'success': False, 'error': f'Failed to write config: {process.stderr}'}

            # Clean up legacy separate server block if it exists
            cls._remove_legacy_site(cls.GITEA_CONFIG_NAME)

            # Reload Nginx
            reload_result = cls.reload()
            if not reload_result['success']:
                return reload_result

            return {
                'success': True,
                'message': 'Gitea nginx config created',
                'config_path': config_path,
                'url_path': '/gitea'
            }

        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def remove_gitea_config(cls) -> Dict:
        """Remove Nginx configuration for Gitea.

        Returns:
            Dict with success status and message
        """
        try:
            # Remove location snippet
            config_path = os.path.join(cls.LOCATIONS_DIR, f'{cls.GITEA_CONFIG_NAME}.conf')
            run_privileged(['rm', '-f', config_path])

            # Clean up legacy separate server block if it exists
            cls._remove_legacy_site(cls.GITEA_CONFIG_NAME)

            # Reload Nginx
            cls.reload()

            return {'success': True, 'message': 'Gitea nginx config removed'}

        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def get_gitea_config(cls) -> Dict:
        """Get the current Gitea nginx configuration.

        Returns:
            Dict with exists, enabled, content, and path
        """
        return cls.get_location_config(cls.GITEA_CONFIG_NAME)

    # ==================== WORDPRESS CONFIGURATION ====================

    @classmethod
    def create_wordpress_config(cls, port: int) -> Dict:
        """Create Nginx location config for WordPress at /wordpress path.

        The config is a location-only snippet included inside the main
        serverkit.conf server block, preventing server_name conflicts.

        Args:
            port: The internal port WordPress is running on

        Returns:
            Dict with success status and message
        """
        try:
            cls._ensure_locations_dir()
            config = cls.WORDPRESS_LOCATION_TEMPLATE.format(port=port)
            config_path = os.path.join(cls.LOCATIONS_DIR, f'{cls.WORDPRESS_CONFIG_NAME}.conf')

            # Write location snippet (no separate server block, no sites-enabled symlink)
            process = run_privileged(['tee', config_path], input=config)
            if process.returncode != 0:
                return {'success': False, 'error': f'Failed to write config: {process.stderr}'}

            # Clean up legacy separate server block if it exists
            cls._remove_legacy_site(cls.WORDPRESS_CONFIG_NAME)

            # Reload Nginx
            reload_result = cls.reload()
            if not reload_result['success']:
                return reload_result

            return {
                'success': True,
                'message': 'WordPress nginx config created',
                'config_path': config_path,
                'url_path': '/wordpress'
            }

        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def remove_wordpress_config(cls) -> Dict:
        """Remove Nginx configuration for WordPress.

        Returns:
            Dict with success status and message
        """
        try:
            # Remove location snippet
            config_path = os.path.join(cls.LOCATIONS_DIR, f'{cls.WORDPRESS_CONFIG_NAME}.conf')
            run_privileged(['rm', '-f', config_path])

            # Clean up legacy separate server block if it exists
            cls._remove_legacy_site(cls.WORDPRESS_CONFIG_NAME)

            # Reload Nginx
            cls.reload()

            return {'success': True, 'message': 'WordPress nginx config removed'}

        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def _remove_legacy_site(cls, name: str):
        """Remove legacy separate server block configs from sites-available/enabled.

        Old versions wrote WordPress/Gitea as full server blocks in sites-available
        with symlinks in sites-enabled. This caused server_name conflicts.
        """
        run_privileged(['rm', '-f', os.path.join(cls.SITES_ENABLED, name)])
        run_privileged(['rm', '-f', os.path.join(cls.SITES_AVAILABLE, name)])

    @classmethod
    def get_wordpress_config(cls) -> Dict:
        """Get the current WordPress nginx configuration.

        Returns:
            Dict with exists, enabled, content, and path
        """
        return cls.get_location_config(cls.WORDPRESS_CONFIG_NAME)
