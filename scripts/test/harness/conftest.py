import os
import time
import uuid
import requests
import pytest

BASE_URL = os.environ.get("SERVERKIT_URL", "http://127.0.0.1:5000")
TIMEOUT = 15


@pytest.fixture(scope="session")
def base_url():
    return BASE_URL


@pytest.fixture(scope="session")
def http():
    s = requests.Session()
    s.headers.update({"Accept": "application/json"})
    return s


@pytest.fixture(scope="session")
def admin_credentials():
    suffix = uuid.uuid4().hex[:8]
    return {
        "email": f"e2e-{suffix}@test.local",
        "username": f"e2e_{suffix}",
        "password": "Test12345!",
    }


@pytest.fixture(scope="session")
def admin_token(http, base_url, admin_credentials):
    """Register the first user (becomes admin) or log in if already exists."""
    # Wait briefly for backend in case test runs immediately after install
    for _ in range(30):
        try:
            r = http.get(f"{base_url}/api/v1/system/health", timeout=2)
            if r.status_code == 200:
                break
        except requests.RequestException:
            time.sleep(1)

    r = http.post(
        f"{base_url}/api/v1/auth/register",
        json=admin_credentials,
        timeout=TIMEOUT,
    )
    if r.status_code in (200, 201):
        return r.json()["access_token"]

    # Fallback: maybe already registered (re-run on same VM)
    r = http.post(
        f"{base_url}/api/v1/auth/login",
        json={
            "email": admin_credentials["email"],
            "password": admin_credentials["password"],
        },
        timeout=TIMEOUT,
    )
    r.raise_for_status()
    return r.json()["access_token"]


@pytest.fixture
def auth_headers(admin_token):
    return {"Authorization": f"Bearer {admin_token}"}
