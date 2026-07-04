"""Tests for app.services.connection_string codec."""

from datetime import datetime

import pytest

from app.services import connection_string as cs


def test_encode_decode_round_trip():
    expires = datetime(2026, 5, 8, 17, 0, 0)
    s = cs.encode(url='https://panel.example.com', token='sk_reg_abc', expires_at=expires)
    assert s.startswith('sk1://')
    assert s == 'sk1://panel.example.com/sk_reg_abc?exp=2026-05-08T17:00:00Z'

    decoded = cs.decode(s)
    assert decoded['url'] == 'https://panel.example.com'
    assert decoded['token'] == 'sk_reg_abc'
    assert decoded['expires_at'] == expires


def test_encode_strips_trailing_slash_from_url():
    # urlparse already discards the trailing slash on a host-only URL,
    # so the reconstructed URL never has one. Lock that in.
    s = cs.encode(url='https://panel.example.com/', token='t', expires_at=None)
    assert cs.decode(s)['url'] == 'https://panel.example.com'


def test_encode_with_no_expiry():
    s = cs.encode(url='https://panel.example.com', token='t', expires_at=None)
    assert s == 'sk1://panel.example.com/t'
    decoded = cs.decode(s)
    assert decoded['expires_at'] is None


def test_encode_preserves_port():
    s = cs.encode(url='https://panel.example.com:9443', token='t', expires_at=None)
    assert s == 'sk1://panel.example.com:9443/t'
    assert cs.decode(s)['url'] == 'https://panel.example.com:9443'


def test_encode_marks_http_as_insecure():
    # http panels (typically dev / local-network) emit insecure=1 so the
    # decoder can reconstruct the right scheme. Without this, an http
    # panel would round-trip as https and the agent would fail TLS.
    s = cs.encode(url='http://localhost:47927', token='t', expires_at=None)
    assert s == 'sk1://localhost:47927/t?insecure=1'
    assert cs.decode(s)['url'] == 'http://localhost:47927'


def test_encode_combines_expiry_and_insecure():
    expires = datetime(2026, 5, 8, 17, 0, 0)
    s = cs.encode(url='http://localhost:47927', token='t', expires_at=expires)
    assert s == 'sk1://localhost:47927/t?exp=2026-05-08T17:00:00Z&insecure=1'
    decoded = cs.decode(s)
    assert decoded['url'] == 'http://localhost:47927'
    assert decoded['expires_at'] == expires


def test_decode_strips_whitespace():
    # Clipboard pastes pick up trailing newlines all the time.
    s = cs.encode(url='https://x', token='t', expires_at=None)
    assert cs.decode('\n  ' + s + '  \n')['token'] == 't'


def test_decode_rejects_missing_version_prefix():
    with pytest.raises(ValueError, match='sk1'):
        cs.decode('not_a_connection_string')


def test_decode_rejects_old_v1_format():
    # The pre-sk1 base64-blob format must not silently decode — agents
    # speaking the new protocol need to reject old payloads cleanly so
    # the user knows to regenerate from a freshly-deployed panel.
    with pytest.raises(ValueError, match='sk1'):
        cs.decode('sk_conn_v1.eyJ1cmwiOiJodHRwczovL3gifQ')


def test_decode_rejects_unknown_scheme():
    with pytest.raises(ValueError, match='sk1'):
        cs.decode('sk2://panel.example.com/token')


def test_decode_rejects_missing_host():
    with pytest.raises(ValueError, match='host'):
        cs.decode('sk1:///token')


def test_decode_rejects_missing_token():
    with pytest.raises(ValueError, match='token'):
        cs.decode('sk1://panel.example.com')
    with pytest.raises(ValueError, match='token'):
        cs.decode('sk1://panel.example.com/')


def test_decode_rejects_token_with_path_separator():
    # Tokens are url-safe by construction (secrets.token_urlsafe), so a
    # slash in the path means a mangled string — better to fail loudly.
    with pytest.raises(ValueError, match='/'):
        cs.decode('sk1://panel.example.com/sk_reg/abc')


def test_decode_rejects_empty():
    with pytest.raises(ValueError):
        cs.decode('')
    with pytest.raises(ValueError):
        cs.decode('   ')


def test_decode_handles_iso_with_z_suffix():
    # encode() emits ISO with a trailing Z. fromisoformat() didn't accept
    # that until 3.11, so the codec strips it explicitly — guard against
    # regressions if anyone touches that branch.
    expires = datetime(2026, 5, 8, 17, 0, 0)
    s = cs.encode(url='https://x', token='t', expires_at=expires)
    assert cs.decode(s)['expires_at'] == expires


def test_encode_rejects_empty_inputs():
    with pytest.raises(ValueError):
        cs.encode(url='', token='t', expires_at=None)
    with pytest.raises(ValueError):
        cs.encode(url='https://x', token='', expires_at=None)


def test_encode_rejects_non_http_scheme():
    with pytest.raises(ValueError, match='http or https'):
        cs.encode(url='ftp://panel.example.com', token='t', expires_at=None)


def test_encode_rejects_url_without_scheme():
    with pytest.raises(ValueError, match='scheme and host'):
        cs.encode(url='panel.example.com', token='t', expires_at=None)


def test_encode_rejects_token_with_slash():
    with pytest.raises(ValueError, match='/'):
        cs.encode(url='https://x', token='bad/token', expires_at=None)
