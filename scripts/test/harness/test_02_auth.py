def test_setup_status_endpoint(http, base_url):
    r = http.get(f"{base_url}/api/v1/auth/setup-status", timeout=10)
    assert r.status_code == 200, r.text
    body = r.json()
    assert "needs_setup" in body, body


def test_admin_register_and_login(admin_token, http, base_url, admin_credentials):
    # admin_token fixture already did the register; verify login works too
    assert admin_token, "no token from register"
    r = http.post(
        f"{base_url}/api/v1/auth/login",
        json={
            "email": admin_credentials["email"],
            "password": admin_credentials["password"],
        },
        timeout=10,
    )
    assert r.status_code == 200, r.text
    assert "access_token" in r.json()


def test_authed_request(http, base_url, auth_headers):
    r = http.get(
        f"{base_url}/api/v1/auth/setup-status",
        headers=auth_headers,
        timeout=10,
    )
    assert r.status_code == 200, r.text
