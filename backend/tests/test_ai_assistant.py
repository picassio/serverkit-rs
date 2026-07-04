"""Tests for the core AI assistant primitive (powered by Prompture)."""
import pytest

from prompture.agents.live_events import (
    TextDelta, ToolResult, ToolUseStart, TurnComplete,
)

from app.services import ai_service
from app.services.ai_tool_registry import ai_tool_registry


class FakeUser:
    def __init__(self, role='admin', perms=None):
        self.role = role
        self.id = 1
        self._perms = perms or {}

    def has_permission(self, feature, level='read'):
        if self.role == 'admin':
            return True
        return self._perms.get((feature, level), False)


# --------------------------------------------------------------------------
# LiveEvent -> SSE frame mapping
# --------------------------------------------------------------------------
def test_live_event_to_frame_text_delta():
    name, data = ai_service.live_event_to_frame(TextDelta(text='hello'))
    assert name == 'text_delta'
    assert data == {'text': 'hello'}


def test_live_event_to_frame_tool_use_start():
    name, data = ai_service.live_event_to_frame(ToolUseStart(id='t1', name='core__list_apps'))
    assert name == 'tool_use_start'
    assert data == {'id': 't1', 'name': 'core__list_apps'}


def test_live_event_to_frame_tool_result():
    name, data = ai_service.live_event_to_frame(
        ToolResult(id='t1', name='core__x', output='ok', is_error=False))
    assert name == 'tool_result'
    assert data['id'] == 't1' and data['output'] == 'ok' and data['is_error'] is False


def test_live_event_to_frame_turn_complete():
    name, data = ai_service.live_event_to_frame(TurnComplete(usage={'cost': 0.12}))
    assert name == 'turn_complete'
    assert data['usage'] == {'cost': 0.12}


# --------------------------------------------------------------------------
# Tool registry: register / unregister / reload + RBAC + namespacing
# --------------------------------------------------------------------------
def test_registry_register_and_unregister():
    def dbl(x: int) -> int:
        """Double a number.

        Args:
            x: the number to double.
        """
        return x * 2

    ai_tool_registry.register(name='dbl', func=dbl, plugin_slug='tpA',
                              rbac_feature='docker', rbac_level='read')
    d = ai_tool_registry.get('tpA__dbl')
    assert d is not None
    assert d.qualified_name == 'tpA__dbl'
    assert d.is_write is False
    assert d.parameters['properties']['x']['type'] == 'integer'

    ai_tool_registry.unregister_plugin('tpA')
    assert ai_tool_registry.get('tpA__dbl') is None


def test_registry_rbac_and_mode_filter():
    def peek() -> str:
        """Peek at something secret."""
        return 'ok'

    ai_tool_registry.register(name='peek', func=peek, plugin_slug='tpB',
                              rbac_feature='security', rbac_level='read')
    try:
        admin = FakeUser(role='admin')
        viewer = FakeUser(role='viewer', perms={('security', 'read'): False})

        admin_names = [d.qualified_name for d in ai_tool_registry.list_for(admin, 'assistant')]
        viewer_names = [d.qualified_name for d in ai_tool_registry.list_for(viewer, 'assistant')]
        assert 'tpB__peek' in admin_names
        assert 'tpB__peek' not in viewer_names

        # Simple mode never offers tools.
        assert ai_tool_registry.list_for(admin, 'simple') == []
    finally:
        ai_tool_registry.unregister_plugin('tpB')


def test_plugin_binder_namespaces_and_flags():
    from app.plugins_sdk.ai import PluginToolBinder
    binder = PluginToolBinder('demo')

    @binder.tool(rbac_feature='git', is_write=True)
    def delete_branch(repo: str, branch: str) -> dict:
        """Delete a branch.

        Args:
            repo: repository slug.
            branch: branch name.
        """
        return {}

    try:
        d = ai_tool_registry.get('demo__delete_branch')
        assert d is not None
        assert d.plugin_slug == 'demo'
        assert d.is_write is True
        assert d.rbac_level == 'write'
    finally:
        ai_tool_registry.unregister_plugin('demo')


# --------------------------------------------------------------------------
# Provider environment construction (Prompture ProviderEnvironment)
# --------------------------------------------------------------------------
def test_build_provider_env_openai_key(monkeypatch):
    monkeypatch.setattr(ai_service, '_setting',
                        lambda k, d=None: {'ai_provider': 'openai', 'ai_endpoint': ''}.get(k, d))
    monkeypatch.setattr(ai_service, '_decrypted_key', lambda: 'sk-test-123')
    env = ai_service.build_provider_env()
    assert env.openai_api_key == 'sk-test-123'
    assert env.ollama_endpoint is None


def test_build_provider_env_ollama_endpoint(monkeypatch):
    monkeypatch.setattr(ai_service, '_setting',
                        lambda k, d=None: {'ai_provider': 'ollama', 'ai_endpoint': 'http://h:11434'}.get(k, d))
    monkeypatch.setattr(ai_service, '_decrypted_key', lambda: None)
    env = ai_service.build_provider_env()
    assert env.ollama_endpoint == 'http://h:11434'
    assert env.openai_api_key is None


def test_summarize_action_includes_params():
    class D:
        name = 'restart_docker_container'
        description = 'Restart a Docker container. STATE-CHANGING'

    summary = ai_service.summarize_action(D(), {'container_id': 'nginx'})
    assert 'container_id=nginx' in summary


# --------------------------------------------------------------------------
# API: RBAC gating + secret never leaked
# --------------------------------------------------------------------------
def test_status_requires_auth_and_reports_configured(client, auth_headers):
    r = client.get('/api/v1/ai/status', headers=auth_headers)
    assert r.status_code == 200
    body = r.get_json()
    assert 'configured' in body and 'enabled' in body


def test_settings_get_never_returns_api_key(client, auth_headers, app):
    from app.services.settings_service import SettingsService
    with app.app_context():
        SettingsService.set('ai_api_key_encrypted', 'ENCRYPTED_SECRET_XYZ')

    r = client.get('/api/v1/ai/settings', headers=auth_headers)
    assert r.status_code == 200
    body = r.get_json()
    assert body['api_key_set'] is True
    assert 'api_key' not in body
    assert 'ENCRYPTED_SECRET_XYZ' not in r.get_data(as_text=True)


def test_settings_put_requires_admin(client, app):
    from app import db
    from app.models import User
    from flask_jwt_extended import create_access_token
    from werkzeug.security import generate_password_hash

    with app.app_context():
        viewer = User(
            email='viewer@test.local', username='viewer_ai',
            password_hash=generate_password_hash('x'),
            role=User.ROLE_VIEWER, is_active=True,
        )
        db.session.add(viewer)
        db.session.commit()
        token = create_access_token(identity=viewer.id)

    r = client.put('/api/v1/ai/settings',
                   headers={'Authorization': f'Bearer {token}'},
                   json={'provider': 'openai'})
    assert r.status_code == 403
