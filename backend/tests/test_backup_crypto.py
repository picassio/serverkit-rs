"""Proving tests for client-side backup encryption.

Backup artifacts can be Fernet-encrypted with ``SERVERKIT_ENCRYPTION_KEY`` before
they ever touch remote storage. These tests prove:
  (a) encrypt/decrypt round-trips the exact original bytes, replacing the
      plaintext with a ``.enc`` artifact,
  (b) a wrong key cannot decrypt (key loss = unrecoverable backups),
  (c) ``encryption_available`` and ``is_encrypted_backup`` report correctly,
  (d) BackupService.backup_files with encryption enabled writes an encrypted
      artifact (unreadable as gzip) whose metadata records ``encrypted: True``,
  (e) ``_finalize_backup`` is a no-op when encryption is disabled.

A valid Fernet key for the env is produced with ``Fernet.generate_key()``.
"""
import gzip
import json
import os
import tarfile

import pytest
from cryptography.fernet import Fernet, InvalidToken

from app.utils import backup_crypto


# ── file-level crypto primitives ────────────────────────────────────────────

def test_encrypt_decrypt_roundtrip(tmp_path, monkeypatch):
    monkeypatch.setenv('SERVERKIT_ENCRYPTION_KEY', Fernet.generate_key().decode())

    src = tmp_path / 'dump.sql'
    original = os.urandom(4096) + b'\n-- some sql --\n'
    src.write_bytes(original)

    enc_path = backup_crypto.encrypt_file(str(src))

    # plaintext is gone, replaced by the .enc artifact
    assert enc_path == str(src) + '.enc'
    assert os.path.exists(enc_path)
    assert not os.path.exists(str(src))
    # ciphertext on disk is not the original bytes
    assert open(enc_path, 'rb').read() != original

    dec_path = backup_crypto.decrypt_file(enc_path)
    assert dec_path == str(src)            # trailing .enc stripped
    assert os.path.exists(dec_path)
    assert open(dec_path, 'rb').read() == original
    # decrypt leaves the source .enc in place
    assert os.path.exists(enc_path)


def test_decrypt_to_explicit_dest(tmp_path, monkeypatch):
    monkeypatch.setenv('SERVERKIT_ENCRYPTION_KEY', Fernet.generate_key().decode())

    src = tmp_path / 'config.tar.gz'
    src.write_bytes(b'payload-bytes')
    enc_path = backup_crypto.encrypt_file(str(src))

    dest = tmp_path / 'out.bin'
    out = backup_crypto.decrypt_file(enc_path, dest=str(dest))
    assert out == str(dest)
    assert dest.read_bytes() == b'payload-bytes'


def test_decrypt_with_wrong_key_raises(tmp_path, monkeypatch):
    monkeypatch.setenv('SERVERKIT_ENCRYPTION_KEY', Fernet.generate_key().decode())

    src = tmp_path / 'secret.sql'
    src.write_bytes(b'sensitive backup contents')
    enc_path = backup_crypto.encrypt_file(str(src))

    # rotate to a different valid Fernet key -> decryption must fail
    monkeypatch.setenv('SERVERKIT_ENCRYPTION_KEY', Fernet.generate_key().decode())
    with pytest.raises(InvalidToken):
        backup_crypto.decrypt_file(enc_path, dest=str(tmp_path / 'out.bin'))


def test_encryption_available_reflects_env(monkeypatch):
    monkeypatch.delenv('SERVERKIT_ENCRYPTION_KEY', raising=False)
    assert backup_crypto.encryption_available() is False

    monkeypatch.setenv('SERVERKIT_ENCRYPTION_KEY', Fernet.generate_key().decode())
    assert backup_crypto.encryption_available() is True


def test_is_encrypted_backup():
    assert backup_crypto.is_encrypted_backup('/var/backups/db.sql.gz.enc') is True
    assert backup_crypto.is_encrypted_backup('/var/backups/db.sql.gz') is False
    assert backup_crypto.is_encrypted_backup('') is False


# ── BackupService integration ───────────────────────────────────────────────

@pytest.fixture
def backup_dirs(tmp_path, monkeypatch):
    """Point BackupService at a throwaway base/config dir and stub uploads."""
    from app.services.backup_service import BackupService

    base = tmp_path / 'backups'
    cfg = tmp_path / 'config'
    base.mkdir()
    cfg.mkdir()

    monkeypatch.setattr(BackupService, 'BACKUP_BASE_DIR', str(base))
    monkeypatch.setattr(BackupService, 'CONFIG_DIR', str(cfg))
    monkeypatch.setattr(BackupService, 'BACKUP_CONFIG', str(cfg / 'backups.json'))
    # never reach out to remote storage in tests
    monkeypatch.setattr(BackupService, '_auto_upload', classmethod(lambda cls, p, info: None))
    return BackupService


def test_backup_files_encrypts_artifact_and_metadata(backup_dirs, tmp_path, monkeypatch):
    monkeypatch.setenv('SERVERKIT_ENCRYPTION_KEY', Fernet.generate_key().decode())
    BackupService = backup_dirs

    BackupService.save_config({'encrypt_backups': True})

    # something to back up
    payload = tmp_path / 'data.txt'
    payload.write_text('hello world ' * 100)

    res = BackupService.backup_files([str(payload)], backup_name='enc')
    assert res['success'] is True

    artifact = res['backup']['path']
    assert artifact.endswith('.enc')
    assert os.path.exists(artifact)
    assert res['backup']['encrypted'] is True

    # the artifact is NOT a readable gzip/tar until decrypted
    with pytest.raises(Exception):
        with tarfile.open(artifact, 'r:gz'):
            pass

    # metadata sidecar records encryption + the encrypted path.
    # backup_name is suffixed with a timestamp; find the sidecar dynamically.
    files_dir = os.path.join(BackupService.BACKUP_BASE_DIR, 'files')
    sidecars = [f for f in os.listdir(files_dir) if f.endswith('.json')]
    assert len(sidecars) == 1
    meta = json.loads(open(os.path.join(files_dir, sidecars[0])).read())
    assert meta['encrypted'] is True
    assert meta['path'].endswith('.enc')

    # decrypting yields a valid gzip tar again
    dec = backup_crypto.decrypt_file(artifact)
    with tarfile.open(dec, 'r:gz') as tar:
        names = tar.getnames()
    assert 'data.txt' in names


def test_finalize_backup_noop_when_disabled(backup_dirs, tmp_path):
    BackupService = backup_dirs
    BackupService.save_config({'encrypt_backups': False})

    f = tmp_path / 'plain.sql.gz'
    f.write_bytes(b'not encrypted')
    info = {'path': str(f), 'size': f.stat().st_size}

    out = BackupService._finalize_backup(str(f), info)
    assert out == str(f)
    assert info['encrypted'] is False
    assert os.path.exists(str(f))


def test_finalize_backup_encrypts_when_enabled(backup_dirs, tmp_path, monkeypatch):
    monkeypatch.setenv('SERVERKIT_ENCRYPTION_KEY', Fernet.generate_key().decode())
    BackupService = backup_dirs
    BackupService.save_config({'encrypt_backups': True})

    f = tmp_path / 'plain.sql.gz'
    f.write_bytes(b'db dump bytes')
    info = {'path': str(f), 'size': f.stat().st_size}

    out = BackupService._finalize_backup(str(f), info)
    assert out.endswith('.enc')
    assert info['encrypted'] is True
    assert info['path'] == out
    assert info['size'] == os.path.getsize(out)
    assert not os.path.exists(str(f))


def test_encrypt_backups_default_roundtrips_through_config(backup_dirs):
    BackupService = backup_dirs
    # default config exposes the flag (False)
    assert BackupService.get_config()['encrypt_backups'] is False
    # and it persists through save/get
    cfg = BackupService.get_config()
    cfg['encrypt_backups'] = True
    BackupService.save_config(cfg)
    assert BackupService.get_config()['encrypt_backups'] is True
