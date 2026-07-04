"""Tests for NVIDIA GPU monitoring (nvidia-smi parsing)."""
import types

from app.services.gpu_service import GpuService

SAMPLE = "0, NVIDIA GeForce RTX 3090, 15, 2048, 24576, 45, 120.50, 350.00, 30, 535.104.05\n"


def _fake_run(stdout, rc=0):
    return lambda *a, **k: types.SimpleNamespace(returncode=rc, stdout=stdout, stderr='')


class TestGpuService:
    def test_list_gpus_parses_a_row(self, monkeypatch):
        monkeypatch.setattr(GpuService, '_run', _fake_run(SAMPLE))
        gpus = GpuService.list_gpus()
        assert len(gpus) == 1
        g = gpus[0]
        assert g['index'] == 0
        assert g['name'] == 'NVIDIA GeForce RTX 3090'
        assert g['memory_total'] == 24576.0
        assert g['memory_percent'] == round(100 * 2048 / 24576, 1)
        assert g['driver_version'] == '535.104.05'

    def test_coerces_na_to_none(self, monkeypatch):
        line = "0, GPU, [N/A], 100, 1000, [N/A], 50, 100, [N/A], 535\n"
        monkeypatch.setattr(GpuService, '_run', _fake_run(line))
        g = GpuService.list_gpus()[0]
        assert g['utilization_gpu'] is None
        assert g['temperature'] is None
        assert g['fan_speed'] is None
        assert g['memory_percent'] == 10.0

    def test_available_false_on_nonzero_exit(self, monkeypatch):
        monkeypatch.setattr(GpuService, '_run', _fake_run('', rc=1))
        assert GpuService.available() is False

    def test_info_when_no_gpus(self, monkeypatch):
        monkeypatch.setattr(GpuService, '_run', _fake_run('', rc=1))
        info = GpuService.info()
        assert info['available'] is False
        assert info['gpus'] == [] and info['processes'] == []


class TestGpuApi:
    def test_gpu_endpoint_returns_info(self, client, auth_headers, app, monkeypatch):
        monkeypatch.setattr(GpuService, 'list_gpus', lambda: [{'index': 0, 'name': 'RTX 3090'}])
        monkeypatch.setattr(GpuService, 'processes', lambda: [])
        resp = client.get('/api/v1/gpu/', headers=auth_headers)
        assert resp.status_code == 200
        body = resp.get_json()
        assert body['available'] is True
        assert len(body['gpus']) == 1

    def test_gpu_requires_auth(self, client, app):
        assert client.get('/api/v1/gpu/').status_code == 401
