"""Lightweight smoke tests on a sampling of authed endpoints.
Goal: catch import-time failures and 500s from blueprint registration."""

import pytest

# Endpoints we expect to exist and return something sane (200 or 4xx, not 5xx).
# Don't expand wildly — keep this focused on "did the blueprint load."
ENDPOINTS = [
    "/api/v1/servers",
    "/api/v1/plugins",
    "/api/v1/system/health",
    "/api/v1/auth/setup-status",
]


@pytest.mark.parametrize("path", ENDPOINTS)
def test_endpoint_no_5xx(http, base_url, auth_headers, path):
    r = http.get(f"{base_url}{path}", headers=auth_headers, timeout=15)
    assert r.status_code < 500, f"{path} returned {r.status_code}: {r.text[:300]}"
