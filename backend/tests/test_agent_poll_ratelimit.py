"""
Regression test for the poll-transport auth rate-limit bypass (bug #3).

The WebSocket auth path (agent_gateway.on_auth) is throttled per-IP via
_check_auth_rate_limit, but the REST long-poll equivalent
(POST /api/v1/agent/connect) did full HMAC auth WITHOUT the throttle — an
unthrottled credential-stuffing surface. The fix applies the same shared
limiter to /connect.
"""
import pytest

import app.agent_gateway as gw


@pytest.fixture(autouse=True)
def _reset_rate_limiter():
    gw._auth_attempts.clear()
    yield
    gw._auth_attempts.clear()


def _bogus_auth():
    # All required fields present so we pass the 400 "missing fields" check and
    # reach the rate limiter; creds are invalid so auth itself would 401.
    return {
        'agent_id': 'nope',
        'api_key_prefix': 'sk_nope12345',
        'signature': 'x' * 16,
        'timestamp': 1,
    }


def test_poll_connect_is_rate_limited(client):
    url = '/api/v1/agent/connect'

    # The first N attempts clear the throttle (and fail auth with 401).
    for _ in range(gw._AUTH_RATE_LIMIT):
        r = client.post(url, json=_bogus_auth())
        assert r.status_code == 401, r.get_data(as_text=True)

    # The next attempt from the same IP is throttled, not auth-checked.
    r = client.post(url, json=_bogus_auth())
    assert r.status_code == 429
