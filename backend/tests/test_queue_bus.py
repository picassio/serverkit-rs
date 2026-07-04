"""Tests for the ServerKit Queue Bus."""
import pytest
from datetime import datetime, timedelta

from app import db
from app.queue_bus.service import QueueBusService, QueueBusError
from app.queue_bus.models import QueueGroup, Queue, QueueMessage


@pytest.fixture(autouse=True)
def reset_broker(app):
    """Reset the service broker singleton before each test."""
    QueueBusService.reset_broker()


def _create_group_queue(group='test-group', queue='test-queue'):
    QueueBusService.create_group(group, name='Test Group')
    QueueBusService.create_queue(group, queue, name='Test Queue', config={'max_attempts': 3})


class TestGroups:
    def test_create_group(self, app):
        group = QueueBusService.create_group('my-group', name='My Group')
        assert group['slug'] == 'my-group'
        assert group['name'] == 'My Group'
        assert QueueGroup.query.filter_by(slug='my-group').first() is not None

    def test_create_group_from_name(self, app):
        group = QueueBusService.create_group(name='My Queue Group!')
        assert group['slug'] == 'my-queue-group'
        assert group['name'] == 'My Queue Group!'

    def test_create_group_duplicate_gets_unique_slug(self, app):
        QueueBusService.create_group('my-group')
        group = QueueBusService.create_group('my-group')
        assert group['slug'] == 'my-group-1'

    def test_list_groups(self, app):
        QueueBusService.create_group('g1')
        QueueBusService.create_group('g2')
        groups = QueueBusService.list_groups()
        assert len(groups) == 2

    def test_update_group(self, app):
        QueueBusService.create_group('g1')
        updated = QueueBusService.update_group('g1', name='Updated', config={'retention_hours': 24})
        assert updated['name'] == 'Updated'
        assert updated['config'] == {'retention_hours': 24}

    def test_delete_group_cascades(self, app):
        _create_group_queue('g1', 'q1')
        QueueBusService.send('g1', 'q1', {'hello': 'world'})
        QueueBusService.delete_group('g1')
        assert QueueGroup.query.filter_by(slug='g1').first() is None
        assert Queue.query.filter_by(slug='q1').first() is None
        assert QueueMessage.query.count() == 0


class TestQueues:
    def test_create_queue(self, app):
        QueueBusService.create_group('g1')
        queue = QueueBusService.create_queue('g1', 'q1', name='Q1', config={'visibility_timeout_ms': 60000})
        assert queue['slug'] == 'q1'
        assert queue['config']['visibility_timeout_ms'] == 60000

    def test_create_queue_duplicate_gets_unique_slug(self, app):
        QueueBusService.create_group('g1')
        QueueBusService.create_queue('g1', 'q1')
        queue = QueueBusService.create_queue('g1', 'q1')
        assert queue['slug'] == 'q1-1'

    def test_list_queues(self, app):
        QueueBusService.create_group('g1')
        QueueBusService.create_queue('g1', 'q1')
        QueueBusService.create_queue('g1', 'q2')
        queues = QueueBusService.list_queues('g1')
        assert len(queues) == 2


class TestMessages:
    def test_send_and_receive(self, app):
        _create_group_queue('g1', 'q1')
        QueueBusService.send('g1', 'q1', {'task': 'hello'})
        messages = QueueBusService.receive('g1', 'q1')
        assert len(messages) == 1
        assert messages[0]['payload'] == {'task': 'hello'}
        assert messages[0]['status'] == QueueMessage.STATUS_IN_FLIGHT

    def test_complete_message(self, app):
        _create_group_queue('g1', 'q1')
        QueueBusService.send('g1', 'q1', {'task': 'hello'})
        received = QueueBusService.receive('g1', 'q1')[0]
        completed = QueueBusService.complete('g1', 'q1', received['id'])
        assert completed['status'] == QueueMessage.STATUS_COMPLETED
        assert completed['completed_at'] is not None

    def test_fail_then_retry(self, app):
        _create_group_queue('g1', 'q1')
        QueueBusService.send('g1', 'q1', {'task': 'hello'})
        received = QueueBusService.receive('g1', 'q1')[0]
        failed = QueueBusService.fail('g1', 'q1', received['id'], error_message='boom')
        assert failed['status'] == QueueMessage.STATUS_PENDING
        assert failed['attempts'] == 1
        assert failed['error_message'] is not None

    def test_dead_letter_after_max_attempts(self, app):
        _create_group_queue('g1', 'q1')
        QueueBusService.send('g1', 'q1', {'task': 'hello'}, max_attempts=2)
        m1 = QueueBusService.receive('g1', 'q1')[0]
        # First failure is requeued immediately to bypass the retry delay.
        QueueBusService.fail('g1', 'q1', m1['id'], error_message='attempt 1', requeue=True)
        m2 = QueueBusService.receive('g1', 'q1')[0]
        QueueBusService.fail('g1', 'q1', m2['id'], error_message='attempt 2')
        # No more pending messages.
        assert QueueBusService.receive('g1', 'q1') == []
        message = QueueBusService.get_message('g1', 'q1', m1['id'])
        assert message['status'] == QueueMessage.STATUS_DEAD_LETTER

    def test_visibility_timeout_blocks_re_receive(self, app):
        _create_group_queue('g1', 'q1')
        QueueBusService.send('g1', 'q1', {'task': 'hello'})
        QueueBusService.receive('g1', 'q1', visibility_timeout_ms=60000)
        second = QueueBusService.receive('g1', 'q1')
        assert second == []

    def test_requeue_failed_message(self, app):
        _create_group_queue('g1', 'q1')
        QueueBusService.send('g1', 'q1', {'task': 'hello'}, max_attempts=1)
        m = QueueBusService.receive('g1', 'q1')[0]
        QueueBusService.fail('g1', 'q1', m['id'], error_message='boom')
        requeued = QueueBusService.requeue('g1', 'q1', m['id'])
        assert requeued['status'] == QueueMessage.STATUS_PENDING
        assert QueueBusService.receive('g1', 'q1') != []

    def test_delayed_message(self, app):
        _create_group_queue('g1', 'q1')
        QueueBusService.send('g1', 'q1', {'task': 'hello'}, delay_ms=3600000)
        assert QueueBusService.receive('g1', 'q1') == []

    def test_priority_ordering(self, app):
        _create_group_queue('g1', 'q1')
        QueueBusService.send('g1', 'q1', {'task': 'low'}, priority=0)
        QueueBusService.send('g1', 'q1', {'task': 'high'}, priority=10)
        messages = QueueBusService.receive('g1', 'q1', max_messages=2)
        assert messages[0]['payload']['task'] == 'high'
        assert messages[1]['payload']['task'] == 'low'


class TestApi:
    def test_create_group_via_api(self, client, auth_headers, app):
        resp = client.post('/api/v1/queue/groups', json={
            'slug': 'api-group',
            'name': 'API Group',
        }, headers=auth_headers)
        assert resp.status_code == 201
        assert resp.get_json()['group']['slug'] == 'api-group'

    def test_send_and_receive_via_api(self, client, auth_headers, app):
        client.post('/api/v1/queue/groups', json={'slug': 'g1', 'name': 'G1'}, headers=auth_headers)
        client.post('/api/v1/queue/groups/g1/queues', json={'slug': 'q1', 'name': 'Q1'}, headers=auth_headers)
        send_resp = client.post('/api/v1/queue/groups/g1/queues/q1/messages', json={
            'payload': {'hello': 'world'},
        }, headers=auth_headers)
        assert send_resp.status_code == 201

        receive_resp = client.post('/api/v1/queue/groups/g1/queues/q1/messages/receive', json={
            'max_messages': 1,
        }, headers=auth_headers)
        assert receive_resp.status_code == 200
        data = receive_resp.get_json()
        assert len(data['messages']) == 1
        assert data['messages'][0]['payload'] == {'hello': 'world'}

    def test_complete_via_api(self, client, auth_headers, app):
        client.post('/api/v1/queue/groups', json={'slug': 'g1', 'name': 'G1'}, headers=auth_headers)
        client.post('/api/v1/queue/groups/g1/queues', json={'slug': 'q1', 'name': 'Q1'}, headers=auth_headers)
        client.post('/api/v1/queue/groups/g1/queues/q1/messages', json={'payload': {'x': 1}}, headers=auth_headers)
        msg = client.post('/api/v1/queue/groups/g1/queues/q1/messages/receive', json={}, headers=auth_headers).get_json()['messages'][0]
        resp = client.post(f'/api/v1/queue/groups/g1/queues/q1/messages/{msg["id"]}/complete', json={}, headers=auth_headers)
        assert resp.status_code == 200
        assert resp.get_json()['message']['status'] == QueueMessage.STATUS_COMPLETED

    def test_stats_endpoint(self, client, auth_headers, app):
        client.post('/api/v1/queue/groups', json={'slug': 'g1', 'name': 'G1'}, headers=auth_headers)
        client.post('/api/v1/queue/groups/g1/queues', json={'slug': 'q1', 'name': 'Q1'}, headers=auth_headers)
        client.post('/api/v1/queue/groups/g1/queues/q1/messages', json={'payload': {'x': 1}}, headers=auth_headers)
        resp = client.get('/api/v1/queue/stats', headers=auth_headers)
        assert resp.status_code == 200
        assert resp.get_json()['messages']['pending'] == 1

    def test_shorthand_send(self, client, auth_headers, app):
        client.post('/api/v1/queue/groups', json={'slug': 'g1', 'name': 'G1'}, headers=auth_headers)
        client.post('/api/v1/queue/groups/g1/queues', json={'slug': 'q1', 'name': 'Q1'}, headers=auth_headers)
        resp = client.post('/api/v1/queue/g1/q1/messages', json={'payload': {'x': 1}}, headers=auth_headers)
        assert resp.status_code == 201
