import os
import subprocess
import json
from datetime import datetime
from typing import Dict, List, Optional
from pathlib import Path

from app.utils.system import ServiceControl, run_privileged, PackageManager, is_command_available


class SSLService:
    """Service for SSL certificate management with Let's Encrypt."""

    CERTBOT_BIN = os.environ.get('CERTBOT_BIN', '/usr/bin/certbot')
    CERTS_DIR = '/etc/letsencrypt/live'
    RENEWAL_DIR = '/etc/letsencrypt/renewal'

    @classmethod
    def is_certbot_installed(cls) -> bool:
        """Check if certbot is installed."""
        return is_command_available('certbot')

    @classmethod
    def install_certbot(cls) -> Dict:
        """Install certbot if not present."""
        if not PackageManager.is_available():
            return {'success': False, 'error': 'No supported package manager found'}

        try:
            result = PackageManager.install(
                ['certbot', 'python3-certbot-nginx'],
                timeout=300,
            )
            if result.returncode != 0:
                return {'success': False, 'error': result.stderr}

            return {'success': True, 'message': 'Certbot installed successfully'}
        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def obtain_certificate(cls, domains: List[str], email: str,
                           webroot_path: str = None, use_nginx: bool = True) -> Dict:
        """Obtain a new SSL certificate from Let's Encrypt."""
        if not cls.is_certbot_installed():
            install_result = cls.install_certbot()
            if not install_result['success']:
                return install_result

        try:
            # Build certbot command
            cmd = [cls.CERTBOT_BIN, 'certonly']

            if use_nginx:
                cmd.append('--nginx')
            elif webroot_path:
                cmd.extend(['--webroot', '-w', webroot_path])
            else:
                return {'success': False, 'error': 'Either use_nginx or webroot_path is required'}

            # Add domains
            for domain in domains:
                cmd.extend(['-d', domain])

            # Add email and agree to TOS
            cmd.extend([
                '--email', email,
                '--agree-tos',
                '--non-interactive',
                '--expand'
            ])

            result = run_privileged(cmd, timeout=300)

            if result.returncode == 0:
                primary_domain = domains[0]
                cert_path = f'{cls.CERTS_DIR}/{primary_domain}/fullchain.pem'
                key_path = f'{cls.CERTS_DIR}/{primary_domain}/privkey.pem'

                response = {
                    'success': True,
                    'message': 'Certificate obtained successfully',
                    'certificate_path': cert_path,
                    'private_key_path': key_path,
                    'domains': domains
                }

                # Best-effort: authorize Let's Encrypt via a CAA record on whichever
                # connected DNS provider manages the domain. This satisfies CAA
                # security scanners and pins issuance to our CA. Never let a CAA
                # hiccup fail an otherwise-successful certificate.
                try:
                    from app.services.dns_provider_service import DNSProviderService
                    response['caa'] = DNSProviderService.ensure_caa_record(primary_domain)
                except Exception as e:
                    response['caa'] = {'created': False, 'reason': 'error', 'error': str(e)}

                return response
            else:
                return {'success': False, 'error': result.stderr}

        except subprocess.TimeoutExpired:
            return {'success': False, 'error': 'Certificate request timed out'}
        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def renew_certificate(cls, domain: str = None) -> Dict:
        """Renew SSL certificate(s)."""
        try:
            cmd = [cls.CERTBOT_BIN, 'renew', '--non-interactive']

            if domain:
                cmd.extend(['--cert-name', domain])

            result = run_privileged(cmd, timeout=300)

            return {
                'success': result.returncode == 0,
                'message': result.stdout if result.returncode == 0 else result.stderr
            }
        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def revoke_certificate(cls, domain: str) -> Dict:
        """Revoke an SSL certificate."""
        cert_path = f'{cls.CERTS_DIR}/{domain}/fullchain.pem'

        try:
            cmd = [
                cls.CERTBOT_BIN, 'revoke',
                '--cert-path', cert_path,
                '--non-interactive'
            ]

            result = run_privileged(cmd, timeout=120)

            if result.returncode == 0:
                # Also delete the certificate
                delete_cmd = [
                    cls.CERTBOT_BIN, 'delete',
                    '--cert-name', domain,
                    '--non-interactive'
                ]
                run_privileged(delete_cmd)

                return {'success': True, 'message': f'Certificate for {domain} revoked and deleted'}

            return {'success': False, 'error': result.stderr}
        except Exception as e:
            return {'success': False, 'error': str(e)}

    @classmethod
    def list_certificates(cls) -> List[Dict]:
        """List all installed certificates."""
        certificates = []

        try:
            result = run_privileged([cls.CERTBOT_BIN, 'certificates'], timeout=60)

            if result.returncode != 0:
                return certificates

            # Parse certbot output
            current_cert = None
            for line in result.stdout.split('\n'):
                line = line.strip()

                if line.startswith('Certificate Name:'):
                    if current_cert:
                        certificates.append(current_cert)
                    current_cert = {'name': line.split(':', 1)[1].strip()}

                elif current_cert:
                    if line.startswith('Domains:'):
                        current_cert['domains'] = line.split(':', 1)[1].strip().split()
                    elif line.startswith('Expiry Date:'):
                        expiry_str = line.split(':', 1)[1].strip()
                        # Parse expiry date
                        try:
                            # Format: 2024-03-15 12:00:00+00:00
                            expiry_part = expiry_str.split(' (')[0]
                            current_cert['expiry'] = expiry_part
                            current_cert['expiry_valid'] = 'VALID' in expiry_str
                        except Exception:
                            current_cert['expiry'] = expiry_str
                    elif line.startswith('Certificate Path:'):
                        current_cert['cert_path'] = line.split(':', 1)[1].strip()
                    elif line.startswith('Private Key Path:'):
                        current_cert['key_path'] = line.split(':', 1)[1].strip()

            if current_cert:
                certificates.append(current_cert)

        except Exception:
            pass

        return certificates

    @classmethod
    def get_certificate_info(cls, domain: str) -> Optional[Dict]:
        """Get detailed information about a specific certificate."""
        cert_path = f'{cls.CERTS_DIR}/{domain}/fullchain.pem'

        try:
            # Use openssl to get certificate details
            result = run_privileged(
                ['openssl', 'x509', '-in', cert_path, '-noout',
                 '-subject', '-issuer', '-dates', '-serial'],
            )

            if result.returncode != 0:
                return None

            info = {'domain': domain, 'cert_path': cert_path}

            for line in result.stdout.split('\n'):
                if line.startswith('subject='):
                    info['subject'] = line.split('=', 1)[1].strip()
                elif line.startswith('issuer='):
                    info['issuer'] = line.split('=', 1)[1].strip()
                elif line.startswith('notBefore='):
                    info['valid_from'] = line.split('=', 1)[1].strip()
                elif line.startswith('notAfter='):
                    info['valid_until'] = line.split('=', 1)[1].strip()
                elif line.startswith('serial='):
                    info['serial'] = line.split('=', 1)[1].strip()

            return info

        except Exception:
            return None

    @classmethod
    def check_expiry(cls, domain: str) -> Dict:
        """Check if a certificate is expiring soon."""
        try:
            cert_path = f'{cls.CERTS_DIR}/{domain}/fullchain.pem'

            # Check expiry with openssl
            result = run_privileged(
                ['openssl', 'x509', '-in', cert_path, '-checkend', '2592000'],
            )

            expiring_soon = result.returncode != 0

            # Get actual expiry date
            date_result = run_privileged(
                ['openssl', 'x509', '-in', cert_path, '-noout', '-enddate'],
            )

            expiry_date = None
            if date_result.returncode == 0:
                expiry_date = date_result.stdout.replace('notAfter=', '').strip()

            return {
                'domain': domain,
                'expiring_soon': expiring_soon,
                'expiry_date': expiry_date,
                'needs_renewal': expiring_soon
            }

        except Exception as e:
            return {'domain': domain, 'error': str(e)}

    @classmethod
    def setup_auto_renewal(cls) -> Dict:
        """Set up automatic certificate renewal via cron/systemd timer."""
        try:
            # Check if systemd timer exists
            if ServiceControl.is_enabled('certbot.timer'):
                return {'success': True, 'message': 'Auto-renewal already configured via systemd'}

            # Enable systemd timer
            enable_result = run_privileged(
                ['systemctl', 'enable', '--now', 'certbot.timer'],
            )

            if enable_result.returncode == 0:
                return {'success': True, 'message': 'Auto-renewal enabled via systemd timer'}

            # Fall back to cron
            cron_job = '0 0,12 * * * root certbot renew --quiet --post-hook "systemctl reload nginx"'
            cron_file = '/etc/cron.d/certbot-renewal'

            run_privileged(['tee', cron_file], input=cron_job + '\n')

            return {'success': True, 'message': 'Auto-renewal configured via cron'}

        except Exception as e:
            return {'success': False, 'error': str(e)}
