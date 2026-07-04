"""Webhook event subscription endpoints."""
from flask import Blueprint, jsonify, request
from flask_jwt_extended import jwt_required, get_jwt_identity
from app.middleware.rbac import get_current_user, auth_required
from app.models.event_subscription import EventSubscription
from app.services.event_service import EventService
from app import db

event_subscriptions_bp = Blueprint('event_subscriptions', __name__)


@event_subscriptions_bp.route('/', methods=['GET'])
@jwt_required()
def list_subscriptions():
    """List webhook subscriptions."""
    user = get_current_user()
    if not user or not user.is_developer:
        return jsonify({'error': 'Developer access required'}), 403

    if user.is_admin:
        subs = EventSubscription.query.order_by(EventSubscription.created_at.desc()).all()
    else:
        subs = EventSubscription.query.filter_by(user_id=user.id).order_by(
            EventSubscription.created_at.desc()
        ).all()

    return jsonify({'subscriptions': [s.to_dict() for s in subs]})


@event_subscriptions_bp.route('/', methods=['POST'])
@jwt_required()
def create_subscription():
    """Create a new webhook subscription."""
    user = get_current_user()
    if not user or not user.is_developer:
        return jsonify({'error': 'Developer access required'}), 403

    data = request.get_json() or {}
    name = data.get('name')
    url = data.get('url')
    events = data.get('events', [])

    if not name or not url:
        return jsonify({'error': 'Name and URL are required'}), 400
    if not events:
        return jsonify({'error': 'At least one event type is required'}), 400

    sub = EventSubscription(
        user_id=user.id,
        name=name,
        url=url,
        retry_count=data.get('retry_count', 3),
        timeout_seconds=data.get('timeout_seconds', 10),
    )
    sub.set_events(events)

    if data.get('generate_secret', True):
        sub.secret = EventSubscription.generate_secret()

    custom_headers = data.get('headers')
    if custom_headers:
        sub.set_headers(custom_headers)

    db.session.add(sub)
    db.session.commit()

    result = sub.to_dict()
    # Expose secret once at creation
    if sub.secret:
        result['secret'] = sub.secret

    return jsonify(result), 201


@event_subscriptions_bp.route('/events', methods=['GET'])
@auth_required()
def list_events():
    """List available event types."""
    return jsonify({'events': EventService.get_available_events()})


@event_subscriptions_bp.route('/<int:sub_id>', methods=['GET'])
@jwt_required()
def get_subscription(sub_id):
    """Get subscription details."""
    user = get_current_user()
    if not user:
        return jsonify({'error': 'User not found'}), 404

    sub = EventSubscription.query.get(sub_id)
    if not sub:
        return jsonify({'error': 'Subscription not found'}), 404
    if not user.is_admin and sub.user_id != user.id:
        return jsonify({'error': 'Access denied'}), 403

    return jsonify(sub.to_dict())


@event_subscriptions_bp.route('/<int:sub_id>', methods=['PUT'])
@jwt_required()
def update_subscription(sub_id):
    """Update a webhook subscription."""
    user = get_current_user()
    if not user:
        return jsonify({'error': 'User not found'}), 404

    sub = EventSubscription.query.get(sub_id)
    if not sub:
        return jsonify({'error': 'Subscription not found'}), 404
    if not user.is_admin and sub.user_id != user.id:
        return jsonify({'error': 'Access denied'}), 403

    data = request.get_json() or {}

    if 'name' in data:
        sub.name = data['name']
    if 'url' in data:
        sub.url = data['url']
    if 'events' in data:
        sub.set_events(data['events'])
    if 'is_active' in data:
        sub.is_active = data['is_active']
    if 'headers' in data:
        sub.set_headers(data['headers'])
    if 'retry_count' in data:
        sub.retry_count = data['retry_count']
    if 'timeout_seconds' in data:
        sub.timeout_seconds = data['timeout_seconds']

    db.session.commit()
    return jsonify(sub.to_dict())


@event_subscriptions_bp.route('/<int:sub_id>', methods=['DELETE'])
@jwt_required()
def delete_subscription(sub_id):
    """Delete a webhook subscription."""
    user = get_current_user()
    if not user:
        return jsonify({'error': 'User not found'}), 404

    sub = EventSubscription.query.get(sub_id)
    if not sub:
        return jsonify({'error': 'Subscription not found'}), 404
    if not user.is_admin and sub.user_id != user.id:
        return jsonify({'error': 'Access denied'}), 403

    db.session.delete(sub)
    db.session.commit()
    return jsonify({'message': 'Subscription deleted'})


@event_subscriptions_bp.route('/<int:sub_id>/test', methods=['POST'])
@jwt_required()
def test_subscription(sub_id):
    """Send a test event to a subscription."""
    user = get_current_user()
    if not user:
        return jsonify({'error': 'User not found'}), 404

    sub = EventSubscription.query.get(sub_id)
    if not sub:
        return jsonify({'error': 'Subscription not found'}), 404
    if not user.is_admin and sub.user_id != user.id:
        return jsonify({'error': 'Access denied'}), 403

    delivery = EventService.send_test(sub_id)
    if not delivery:
        return jsonify({'error': 'Failed to send test'}), 500

    return jsonify(delivery.to_dict())


@event_subscriptions_bp.route('/<int:sub_id>/deliveries', methods=['GET'])
@jwt_required()
def list_deliveries(sub_id):
    """Get delivery history for a subscription."""
    user = get_current_user()
    if not user:
        return jsonify({'error': 'User not found'}), 404

    sub = EventSubscription.query.get(sub_id)
    if not sub:
        return jsonify({'error': 'Subscription not found'}), 404
    if not user.is_admin and sub.user_id != user.id:
        return jsonify({'error': 'Access denied'}), 403

    page = request.args.get('page', 1, type=int)
    per_page = request.args.get('per_page', 50, type=int)

    pagination = EventService.get_deliveries(sub_id, page, per_page)
    return jsonify({
        'deliveries': [d.to_dict() for d in pagination.items],
        'total': pagination.total,
        'page': page,
        'per_page': per_page,
        'pages': pagination.pages,
    })


@event_subscriptions_bp.route('/<int:sub_id>/deliveries/<int:delivery_id>/retry', methods=['POST'])
@jwt_required()
def retry_delivery(sub_id, delivery_id):
    """Retry a failed delivery."""
    user = get_current_user()
    if not user:
        return jsonify({'error': 'User not found'}), 404

    sub = EventSubscription.query.get(sub_id)
    if not sub:
        return jsonify({'error': 'Subscription not found'}), 404
    if not user.is_admin and sub.user_id != user.id:
        return jsonify({'error': 'Access denied'}), 403

    from app.models.event_subscription import EventDelivery
    delivery = EventDelivery.query.filter_by(id=delivery_id, subscription_id=sub_id).first()
    if not delivery:
        return jsonify({'error': 'Delivery not found'}), 404

    delivery.status = EventDelivery.STATUS_PENDING
    delivery.next_retry_at = None
    db.session.commit()

    EventService.deliver(delivery_id)

    # Refresh
    delivery = EventDelivery.query.get(delivery_id)
    return jsonify(delivery.to_dict())
