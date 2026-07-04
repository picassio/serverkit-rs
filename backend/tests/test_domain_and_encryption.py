"""Tests for canonical domain detection and encryption key bootstrap."""
import os
import re
import tempfile
from pathlib import Path

import pytest
from flask import Request
from werkzeug.test import EnvironBuilder

from app.utils.crypto import (
    generate_encryption_key,
    is_encryption_configured,
    get_encryption_key,
    ensure_encryption_key,
    write_encryption_key_to_env,
)
from app.utils.domain import (
    is_ip_address,
    is_valid_canonical_domain,
    detect_request_domain,
    canonical_origin,
)


def _request(headers=None, **environ_kwargs):
    """Build a Flask Request for header-based tests without the full app."""
    builder = EnvironBuilder('/', headers=headers or {}, **environ_kwargs)
    return Request(builder.get_environ())


class TestEncryptionKey:
    """Encryption key generation and bootstrap."""

    def test_generate_encryption_key_is_valid_fernet(self):
        key = generate_encryption_key()
        assert re.match(r'^[A-Za-z0-9_-]{43}=$', key)

    def test_is_encryption_configured_false_when_missing(self, monkeypatch):
        monkeypatch.delenv('SERVERKIT_ENCRYPTION_KEY', raising=False)
        assert is_encryption_configured() is False

    def test_is_encryption_configured_true_when_set(self, monkeypatch):
        monkeypatch.setenv('SERVERKIT_ENCRYPTION_KEY', generate_encryption_key())
        assert is_encryption_configured() is True

    def test_get_encryption_key_uses_env_in_production(self, monkeypatch):
        key = generate_encryption_key()
        monkeypatch.setenv('SERVERKIT_ENCRYPTION_KEY', key)
        monkeypatch.setenv('FLASK_ENV', 'production')
        assert get_encryption_key() == key.encode()

    def test_ensure_encryption_key_auto_generates_in_production(self, monkeypatch, tmp_path):
        monkeypatch.delenv('SERVERKIT_ENCRYPTION_KEY', raising=False)
        monkeypatch.setenv('FLASK_ENV', 'production')
        env_file = tmp_path / '.env'
        monkeypatch.setenv('SERVERKIT_ENV_FILE', str(env_file))

        key = ensure_encryption_key()
        assert key is not None
        assert is_encryption_configured() is True
        assert 'SERVERKIT_ENCRYPTION_KEY=' in env_file.read_text()

    def test_ensure_encryption_key_returns_none_in_testing(self, monkeypatch):
        monkeypatch.delenv('SERVERKIT_ENCRYPTION_KEY', raising=False)
        monkeypatch.setenv('FLASK_ENV', 'testing')
        assert ensure_encryption_key() is None

    def test_write_encryption_key_to_env_appends_and_updates(self, tmp_path):
        env_file = tmp_path / '.env'
        env_file.write_text('SECRET_KEY=abc\n')

        key = generate_encryption_key()
        write_encryption_key_to_env(key, env_file)
        content = env_file.read_text()
        assert f'SERVERKIT_ENCRYPTION_KEY={key}' in content
        assert content.count('SERVERKIT_ENCRYPTION_KEY') == 1

        key2 = generate_encryption_key()
        write_encryption_key_to_env(key2, env_file)
        content = env_file.read_text()
        assert f'SERVERKIT_ENCRYPTION_KEY={key2}' in content
        assert content.count('SERVERKIT_ENCRYPTION_KEY') == 1


class TestDomainDetection:
    """Canonical domain detection helpers."""

    def test_is_ip_address_recognizes_ipv4(self):
        assert is_ip_address('192.168.1.1') is True
        assert is_ip_address('10.0.0.1:8080') is True

    def test_is_ip_address_recognizes_ipv6(self):
        assert is_ip_address('::1') is True
        assert is_ip_address('[::1]:8080') is True
        assert is_ip_address('2001:db8::1') is True

    def test_is_ip_address_rejects_domains(self):
        assert is_ip_address('serverkit.example.com') is False
        assert is_ip_address('localhost') is False

    def test_is_valid_canonical_domain_rejects_localhost_and_ips(self):
        assert is_valid_canonical_domain('localhost') is False
        assert is_valid_canonical_domain('127.0.0.1') is False
        assert is_valid_canonical_domain('192.168.1.1') is False
        assert is_valid_canonical_domain('::1') is False

    def test_is_valid_canonical_domain_rejects_single_label(self):
        assert is_valid_canonical_domain('serverkit') is False

    def test_is_valid_canonical_domain_accepts_real_domains(self):
        assert is_valid_canonical_domain('serverkit.example.com') is True
        assert is_valid_canonical_domain('panel.builditdesign.com') is True

    def test_canonical_origin_builds_url(self):
        assert canonical_origin('serverkit.example.com', True) == 'https://serverkit.example.com'
        assert canonical_origin('serverkit.example.com', False) == 'http://serverkit.example.com'

    def test_detect_request_domain_uses_host_header(self):
        req = _request(headers={'Host': 'serverkit.example.com'})
        domain, is_https = detect_request_domain(req)
        assert domain == 'serverkit.example.com'
        assert is_https is False

    def test_detect_request_domain_uses_x_forwarded_headers(self):
        req = _request(headers={
            'Host': 'localhost',
            'X-Forwarded-Host': 'panel.example.com',
            'X-Forwarded-Proto': 'https',
        })
        domain, is_https = detect_request_domain(req)
        assert domain == 'panel.example.com'
        assert is_https is True

    def test_detect_request_domain_rejects_ip_hosts(self):
        req = _request(headers={'Host': '146.190.213.37'})
        domain, is_https = detect_request_domain(req)
        assert domain is None
