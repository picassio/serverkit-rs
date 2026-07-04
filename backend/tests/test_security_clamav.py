"""Proving tests for the ClamAV start endpoint.

`install_clamav` does not start the daemon, so the Security Overview posture
exposes a one-click "Start service" fix backed by POST /security/clamav/start.
These tests pin its wiring without touching systemctl: ServiceControl is mocked,
so they run on any OS (the service layer itself is Linux-only).
"""
from types import SimpleNamespace
from unittest.mock import patch


def _completed(returncode=0):
    return SimpleNamespace(returncode=returncode, stdout='', stderr='')


def test_start_clamav_success(client, auth_headers):
    """A daemon that comes up active returns 200 + success."""
    with patch('app.services.security_service.ServiceControl') as svc:
        svc.enable.return_value = _completed()
        svc.start.return_value = _completed()
        svc.is_active.return_value = True

        resp = client.post('/api/v1/security/clamav/start', headers=auth_headers)

    assert resp.status_code == 200
    body = resp.get_json()
    assert body['success'] is True
    svc.start.assert_called()  # we actually attempted the start


def test_start_clamav_failure_when_inactive(client, auth_headers):
    """systemctl returns 0 but the unit never goes active -> 400, no false success."""
    with patch('app.services.security_service.ServiceControl') as svc:
        svc.enable.return_value = _completed()
        svc.start.return_value = _completed(returncode=0)
        svc.is_active.return_value = False

        resp = client.post('/api/v1/security/clamav/start', headers=auth_headers)

    assert resp.status_code == 400
    assert resp.get_json()['success'] is False


def test_start_clamav_requires_auth(client):
    """Unauthenticated callers cannot start services."""
    resp = client.post('/api/v1/security/clamav/start')
    assert resp.status_code in (401, 422)
