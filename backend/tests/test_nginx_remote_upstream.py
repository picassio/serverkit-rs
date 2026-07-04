"""Phase 2 #12: the nginx remote-upstream vhost. Pure template/validation
checks — no nginx or filesystem needed."""

from app.services.nginx_service import NginxService


def test_remote_upstream_template_renders():
    cfg = NginxService.REMOTE_UPSTREAM_TEMPLATE.format(
        name='jellyfin.example.com',
        domains='jellyfin.example.com',
        upstream='10.88.3.2:8096',
    )
    assert 'proxy_pass http://10.88.3.2:8096;' in cfg
    assert 'server_name jellyfin.example.com;' in cfg
    # streaming + websocket essentials (Jellyfin)
    assert 'proxy_buffering off;' in cfg
    assert 'proxy_set_header Upgrade $http_upgrade;' in cfg
    assert "proxy_set_header Connection 'upgrade';" in cfg


def test_create_site_remote_requires_upstream():
    # The 'remote' branch rejects a missing upstream before touching the FS.
    res = NginxService.create_site(
        name='x', app_type='remote', domains=['x.example.com'], root_path='')
    assert res.get('success') is False
    assert 'upstream' in res.get('error', '').lower()
