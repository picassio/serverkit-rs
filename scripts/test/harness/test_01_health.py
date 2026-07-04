import requests


def test_backend_health(http, base_url):
    r = http.get(f"{base_url}/api/v1/system/health", timeout=10)
    assert r.status_code == 200, r.text
    body = r.json()
    assert body.get("status") == "healthy", body


def test_frontend_reachable(base_url):
    # Frontend served by nginx on :80 (not the same as backend BASE_URL)
    url = base_url.replace(":5000", "").rstrip("/") or "http://127.0.0.1"
    r = requests.get(url, timeout=10, allow_redirects=True)
    assert r.status_code < 500, f"frontend returned {r.status_code}"
