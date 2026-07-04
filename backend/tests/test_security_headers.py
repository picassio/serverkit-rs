"""Proving tests — HSTS is gated on the operator's SSL choice.

HTTPS is optional in ServerKit, so the panel must NOT advertise HSTS/preload
(a hard-to-reverse browser commitment) unless the deployment actually
terminates HTTPS. The installer records that via SSL_MODE/HSTS_ENABLED; behind
a proxy Flask can't tell real TLS from a Cloudflare-Flexible edge, so we trust
the recorded choice rather than the request scheme.
"""

import config as cfg


def _make_app(hsts_enabled, debug):
    from flask import Flask
    from app.middleware.security import register_security_headers
    app = Flask(__name__)
    app.debug = debug
    app.config['HSTS_ENABLED'] = hsts_enabled
    register_security_headers(app)

    @app.route('/ping')
    def ping():
        return 'ok'

    return app


def test_hsts_sent_in_secure_production():
    r = _make_app(hsts_enabled=True, debug=False).test_client().get('/ping')
    hsts = r.headers.get('Strict-Transport-Security')
    assert hsts and 'preload' in hsts and 'includeSubDomains' in hsts


def test_hsts_absent_in_insecure_mode():
    r = _make_app(hsts_enabled=False, debug=False).test_client().get('/ping')
    assert 'Strict-Transport-Security' not in r.headers
    # The non-committal hardening headers are still applied.
    assert r.headers.get('X-Content-Type-Options') == 'nosniff'
    assert 'Content-Security-Policy' in r.headers


def test_hsts_absent_in_debug_even_if_secure():
    r = _make_app(hsts_enabled=True, debug=True).test_client().get('/ping')
    assert 'Strict-Transport-Security' not in r.headers


def test_resolve_ssl_mode(monkeypatch):
    monkeypatch.setenv('SERVERKIT_SSL_MODE', 'secure')
    assert cfg._resolve_ssl_mode() == 'secure'
    # Unrecognised value falls through; no /etc/serverkit/ssl-mode on the test
    # box, so it lands on the safe default.
    monkeypatch.setenv('SERVERKIT_SSL_MODE', 'bogus')
    assert cfg._resolve_ssl_mode() == 'insecure'
    monkeypatch.delenv('SERVERKIT_SSL_MODE', raising=False)
    assert cfg._resolve_ssl_mode() == 'insecure'
