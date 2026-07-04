"""Proving tests for provider secret encryption-at-rest (#16/#17/#18).

Secrets for every connection store are Fernet-encrypted in the DB / config and
decrypted only at point-of-use. These tests prove:
  (a) writes store ciphertext, not plaintext,
  (b) reads return the original plaintext,
  (c) the wildcard-HTTPS path (which reads DNS creds across services) gets
      PLAINTEXT, not the ciphertext blob — the regression that prompted this file,
  (d) the legacy-plaintext migration encrypts in place.

Note: an encrypted value only round-trips with a stable key; `decrypt_secret_safe`
returns legacy plaintext unchanged so encrypted and not-yet-migrated values coexist.
"""


# ── crypto primitives ───────────────────────────────────────────────────────

def test_crypto_roundtrip_and_passthrough():
    from app.utils.crypto import encrypt_secret, decrypt_secret_safe, is_encrypted
    enc = encrypt_secret('s3cr3t')
    assert enc != 's3cr3t'
    assert is_encrypted(enc) is True
    assert decrypt_secret_safe(enc) == 's3cr3t'
    # a legacy plaintext value passes through untouched
    assert is_encrypted('legacy-plain') is False
    assert decrypt_secret_safe('legacy-plain') == 'legacy-plain'


# ── DNS providers ───────────────────────────────────────────────────────────

def test_dns_single_secret_encrypted_at_rest(app):
    from app.models.email import DNSProviderConfig
    from app.services.dns_provider_service import DNSProviderService
    from app.utils.crypto import is_encrypted

    res = DNSProviderService.add_provider(name='cf', provider='cloudflare', api_key='supertoken')
    assert res['success'] is True

    row = DNSProviderConfig.query.filter_by(name='cf').first()
    assert row.api_key and row.api_key != 'supertoken'      # not plaintext at rest
    assert is_encrypted(row.api_key) is True
    assert DNSProviderService.decrypted_credentials(row)['api_key'] == 'supertoken'


def test_dns_secret_pair_encrypted_at_rest(app):
    from app.models.email import DNSProviderConfig
    from app.services.dns_provider_service import DNSProviderService
    from app.utils.crypto import is_encrypted

    DNSProviderService.add_provider(name='r53', provider='route53', api_key='AKIA', api_secret='SECRET')
    row = DNSProviderConfig.query.filter_by(name='r53').first()
    assert row.api_key != 'AKIA' and is_encrypted(row.api_key)
    assert row.api_secret != 'SECRET' and is_encrypted(row.api_secret)

    creds = DNSProviderService.decrypted_credentials(row)
    assert creds['api_key'] == 'AKIA'
    assert creds['api_secret'] == 'SECRET'


def test_wildcard_setup_receives_plaintext_creds(app, monkeypatch):
    """Regression for the cross-service read: SitesHttpsService must hand the cert
    issuer DECRYPTED creds, never the encrypted-at-rest blob."""
    from app import db
    from app.models.system_settings import SystemSettings
    from app.services.dns_provider_service import DNSProviderService
    from app.services.advanced_ssl_service import AdvancedSSLService
    from app.services.sites_https_service import SitesHttpsService

    res = DNSProviderService.add_provider(name='r53', provider='route53',
                                          api_key='AKIA-PLAIN', api_secret='SK-PLAIN')
    pid = res['provider']['id']
    SystemSettings.set('server_public_ip', '203.0.113.5', value_type='string')
    db.session.commit()

    monkeypatch.setattr(DNSProviderService, 'ensure_a_record',
                        classmethod(lambda cls, h, ip: {'created': True, 'record': {}}))
    captured = {}
    monkeypatch.setattr(
        AdvancedSSLService, 'issue_wildcard_cert',
        staticmethod(lambda domain, prov, creds, email=None:
                     captured.update(creds=creds) or {'success': True, 'certificate_path': '/x'}))

    out = SitesHttpsService.setup(pid, email='ops@x.com')
    assert out['success'] is True
    # decrypted creds reach the issuer — would be ciphertext under the old bug
    assert captured['creds'] == {'api_key': 'AKIA-PLAIN', 'api_secret': 'SK-PLAIN'}


def test_dns_legacy_plaintext_migration(app):
    from app import db
    from app.models.email import DNSProviderConfig
    from app.services.dns_provider_service import DNSProviderService
    from app.utils.crypto import is_encrypted, decrypt_secret_safe

    # a row written before encryption-at-rest landed (plaintext, bypassing add_provider)
    row = DNSProviderConfig(name='legacy', provider='cloudflare', api_key='plain-key')
    db.session.add(row)
    db.session.commit()
    assert is_encrypted(row.api_key) is False

    n = DNSProviderService.encrypt_legacy_secrets()
    assert n >= 1
    assert is_encrypted(row.api_key) is True
    assert decrypt_secret_safe(row.api_key) == 'plain-key'
    # idempotent — a second pass changes nothing
    assert DNSProviderService.encrypt_legacy_secrets() == 0


# ── cloud providers ─────────────────────────────────────────────────────────

def test_cloud_provider_secret_encrypted_at_rest(app):
    from app.models.cloud_server import CloudProvider
    from app.services.cloud_provisioning_service import CloudProvisioningService
    from app.utils.crypto import is_encrypted, decrypt_secret_safe

    p = CloudProvisioningService.create_provider(
        {'provider_type': 'digitalocean', 'name': 'do', 'api_key': 'do-token-xyz'})
    row = CloudProvider.query.get(p.id)
    assert row.api_key_encrypted != 'do-token-xyz'
    assert is_encrypted(row.api_key_encrypted) is True
    assert decrypt_secret_safe(row.api_key_encrypted) == 'do-token-xyz'
    # the auth header uses the decrypted token, not the stored blob
    assert CloudProvisioningService._auth_headers(row)['Authorization'] == 'Bearer do-token-xyz'


# ── storage (config-file backed) ────────────────────────────────────────────

def test_storage_secret_encrypted_at_rest(app, tmp_path, monkeypatch):
    import json
    from app.services.storage_provider_service import StorageProviderService
    from app.utils.crypto import is_encrypted

    cfg_file = tmp_path / 'storage.json'
    monkeypatch.setattr(StorageProviderService, 'CONFIG_FILE', str(cfg_file))

    StorageProviderService.save_config({
        'provider': 's3',
        's3': {'bucket': 'b', 'region': 'us-east-1', 'access_key': 'AKIA-S',
               'secret_key': 'SEKRET', 'endpoint_url': '', 'path_prefix': 'p'},
    })

    raw = json.loads(cfg_file.read_text())
    assert raw['s3']['secret_key'] != 'SEKRET'              # ciphertext on disk
    assert is_encrypted(raw['s3']['secret_key']) is True
    assert StorageProviderService.get_config()['s3']['secret_key'] == 'SEKRET'  # decrypted for use


# ── registrar connections ───────────────────────────────────────────────────

def test_registrar_secret_encrypted_at_rest(app):
    from app.services.registrar_service import RegistrarService
    from app.utils.crypto import is_encrypted, decrypt_secret

    conn = RegistrarService.add_connection({'provider': 'godaddy', 'name': 'gd',
                                            'api_key': 'gd-key', 'api_secret': 'gd-secret'})
    assert conn.api_key_encrypted != 'gd-key'
    assert is_encrypted(conn.api_key_encrypted) is True
    assert decrypt_secret(conn.api_key_encrypted) == 'gd-key'
    assert decrypt_secret(conn.api_secret_encrypted) == 'gd-secret'
