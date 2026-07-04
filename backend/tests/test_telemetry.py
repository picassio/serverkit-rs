"""Tests for the unified telemetry / system event stream."""
from datetime import datetime, timedelta

import pytest

from app import db
from app.models.system_event import SystemEvent
from app.services.telemetry_service import TelemetryService, generate_correlation_id


class TestTelemetryService:
    def test_emit_event_creates_row(self, app):
        event = TelemetryService.emit(
            source='test',
            event_type='test.event',
            message='Hello telemetry',
            severity='info',
            resource_type='app',
            resource_id=42,
            payload={'hello': 'world'},
            commit=True,
        )
        assert event is not None
        assert event.id is not None
        assert event.source == 'test'
        assert event.event_type == 'test.event'
        assert event.message == 'Hello telemetry'
        assert event.resource_type == 'app'
        assert event.resource_id == '42'
        assert event.get_payload() == {'hello': 'world'}

    def test_emit_redacts_sensitive_payload_keys(self, app):
        event = TelemetryService.emit(
            source='test',
            event_type='test.secret',
            payload={
                'username': 'admin',
                'api_key': 'super-secret',
                'nested': {'password': 'hunter2'},
            },
            commit=True,
        )
        payload = event.get_payload()
        assert payload['username'] == 'admin'
        assert payload['api_key'] == '[redacted]'
        assert payload['nested']['password'] == '[redacted]'

    def test_emit_invalid_severity_defaults_to_info(self, app):
        event = TelemetryService.emit(
            source='test',
            event_type='test.event',
            severity='not-a-severity',
            commit=True,
        )
        assert event.severity == 'info'

    def test_emit_truncates_oversized_payload(self, app):
        # After sanitization each string is capped at MAX_STRING_LENGTH and each
        # dict at MAX_DICT_ITEMS, so many large entries still exceed 32 KiB.
        large_payload = {f'key_{i}': 'x' * 2000 for i in range(50)}
        event = TelemetryService.emit(
            source='test',
            event_type='test.large',
            payload=large_payload,
            commit=True,
        )
        payload = event.get_payload()
        assert payload == {'_truncated': True}

    def test_emit_does_not_raise_on_failure(self, app, monkeypatch):
        monkeypatch.setattr(db.session, 'commit', lambda: (_ for _ in ()).throw(Exception('boom')))
        event = TelemetryService.emit(
            source='test',
            event_type='test.event',
            commit=True,
        )
        assert event is None

    def test_get_events_filter_by_source_and_severity(self, app):
        TelemetryService.emit(source='audit', event_type='audit.login', severity='info', commit=True)
        TelemetryService.emit(source='backup', event_type='backup.failed', severity='error', commit=True)
        TelemetryService.emit(source='backup', event_type='backup.completed', severity='info', commit=True)

        result = TelemetryService.get_events(source='backup', severity='error')
        assert result.total == 1
        assert result.items[0].event_type == 'backup.failed'

    def test_get_events_by_correlation_id(self, app):
        correlation_id = generate_correlation_id()
        TelemetryService.emit(source='backup', event_type='backup.started', correlation_id=correlation_id, commit=True)
        TelemetryService.emit(source='backup', event_type='backup.completed', correlation_id=correlation_id, commit=True)
        TelemetryService.emit(source='backup', event_type='backup.started', commit=True)

        events = TelemetryService.get_events_by_correlation(correlation_id)
        assert len(events) == 2

    def test_get_stats(self, app):
        TelemetryService.emit(source='backup', event_type='backup.completed', severity='info', commit=True)
        TelemetryService.emit(source='backup', event_type='backup.failed', severity='error', commit=True)
        TelemetryService.emit(source='monitoring', event_type='monitoring.alert', severity='warning', commit=True)

        stats = TelemetryService.get_stats(hours=24)
        assert stats['total'] == 3
        assert stats['by_severity']['error'] == 1
        assert stats['by_severity']['warning'] == 1
        assert stats['by_severity']['info'] == 1
        assert stats['by_source']['backup'] == 2
        assert stats['by_source']['monitoring'] == 1

    def test_cleanup_old_events(self, app):
        old = TelemetryService.emit(source='test', event_type='test.old', commit=True)
        old.timestamp = datetime.utcnow() - timedelta(days=100)
        db.session.commit()

        TelemetryService.emit(source='test', event_type='test.new', commit=True)

        deleted = TelemetryService.cleanup_old_events(days=90)
        assert deleted == 1
        assert SystemEvent.query.filter_by(event_type='test.old').first() is None
        assert SystemEvent.query.filter_by(event_type='test.new').first() is not None


class TestTelemetryAPI:
    def test_list_events_requires_auth(self, client):
        response = client.get('/api/v1/telemetry/events')
        assert response.status_code == 401

    def test_list_events(self, app, client, auth_headers):
        TelemetryService.emit(source='audit', event_type='audit.login', severity='info', commit=True)
        TelemetryService.emit(source='backup', event_type='backup.failed', severity='error', commit=True)

        response = client.get('/api/v1/telemetry/events', headers=auth_headers)
        assert response.status_code == 200
        data = response.get_json()
        assert data['total'] == 2
        assert len(data['events']) == 2

    def test_list_events_filter_by_source(self, app, client, auth_headers):
        TelemetryService.emit(source='audit', event_type='audit.login', commit=True)
        TelemetryService.emit(source='backup', event_type='backup.failed', commit=True)

        response = client.get('/api/v1/telemetry/events?source=backup', headers=auth_headers)
        assert response.status_code == 200
        data = response.get_json()
        assert data['total'] == 1
        assert data['events'][0]['source'] == 'backup'

    def test_get_event(self, app, client, auth_headers):
        event = TelemetryService.emit(source='test', event_type='test.event', commit=True)
        response = client.get(f'/api/v1/telemetry/events/{event.id}', headers=auth_headers)
        assert response.status_code == 200
        data = response.get_json()
        assert data['id'] == event.id
        assert data['source'] == 'test'

    def test_get_event_not_found(self, client, auth_headers):
        response = client.get('/api/v1/telemetry/events/99999', headers=auth_headers)
        assert response.status_code == 404

    def test_get_events_by_correlation(self, app, client, auth_headers):
        correlation_id = generate_correlation_id()
        TelemetryService.emit(source='backup', event_type='backup.started', correlation_id=correlation_id, commit=True)
        TelemetryService.emit(source='backup', event_type='backup.completed', correlation_id=correlation_id, commit=True)

        response = client.get(f'/api/v1/telemetry/events/by-correlation/{correlation_id}', headers=auth_headers)
        assert response.status_code == 200
        data = response.get_json()
        assert len(data['events']) == 2

    def test_get_stats_endpoint(self, app, client, auth_headers):
        TelemetryService.emit(source='backup', event_type='backup.completed', commit=True)
        response = client.get('/api/v1/telemetry/stats', headers=auth_headers)
        assert response.status_code == 200
        data = response.get_json()
        assert data['total'] >= 1
        assert 'by_severity' in data
        assert 'by_source' in data

    def test_cleanup_events_admin_only(self, app, client, auth_headers):
        response = client.delete('/api/v1/telemetry/events?days=1', headers=auth_headers)
        assert response.status_code == 200

    def test_emit_test_event_admin_only(self, app, client, auth_headers):
        response = client.post('/api/v1/telemetry/events/test', json={'message': 'test'}, headers=auth_headers)
        assert response.status_code == 201
        data = response.get_json()
        assert data['event_type'] == 'telemetry.test'


class TestTelemetryIntegrations:
    def test_audit_service_emits_telemetry_event(self, app):
        from app.services.audit_service import AuditService
        from app.models import User
        from werkzeug.security import generate_password_hash

        user = User(email='audit@test.local', username='audituser', password_hash=generate_password_hash('x'))
        db.session.add(user)
        db.session.commit()

        AuditService.log(
            action='app.create',
            user_id=user.id,
            target_type='app',
            target_id=123,
            details={'name': 'my-app'},
            commit=True,
        )

        event = SystemEvent.query.filter_by(source='audit').first()
        assert event is not None
        assert event.event_type == 'audit.action_logged'
        assert event.actor_user_id == user.id
        assert event.get_payload()['details']['name'] == 'my-app'
