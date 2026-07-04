from flask import Blueprint, request, jsonify
from flask_jwt_extended import jwt_required, get_jwt_identity
from app.middleware.rbac import admin_required
from app.models import User, NotificationPreferences
from app import db
from app.services.notification_service import NotificationService
from app.notifications.service import NotificationBusService
from app.notifications.providers import EmailProviderService, SUPPORTED as EMAIL_PROVIDERS

notifications_bp = Blueprint('notifications', __name__)


@notifications_bp.route('/status', methods=['GET'])
@jwt_required()
def get_status():
    """Get notification channels status."""
    status = NotificationService.get_status()
    return jsonify(status), 200


@notifications_bp.route('/config', methods=['GET'])
@jwt_required()
@admin_required
def get_config():
    """Get notification configuration."""
    config = NotificationService.get_config()

    # Mask sensitive data
    if 'discord' in config and config['discord'].get('webhook_url'):
        url = config['discord']['webhook_url']
        config['discord']['webhook_url'] = url[:30] + '...' if len(url) > 30 else url

    if 'slack' in config and config['slack'].get('webhook_url'):
        url = config['slack']['webhook_url']
        config['slack']['webhook_url'] = url[:30] + '...' if len(url) > 30 else url

    if 'telegram' in config and config['telegram'].get('bot_token'):
        config['telegram']['bot_token'] = '***'

    if 'generic_webhook' in config and config['generic_webhook'].get('url'):
        url = config['generic_webhook']['url']
        config['generic_webhook']['url'] = url[:30] + '...' if len(url) > 30 else url

    if 'email' in config:
        if config['email'].get('smtp_password'):
            config['email']['smtp_password'] = '***'

    return jsonify(config), 200


@notifications_bp.route('/config/<channel>', methods=['PUT'])
@jwt_required()
@admin_required
def update_channel_config(channel):
    """Update configuration for a specific notification channel."""
    data = request.get_json()
    if not data:
        return jsonify({'error': 'No data provided'}), 400

    valid_channels = ['discord', 'slack', 'telegram', 'email', 'generic_webhook']
    if channel not in valid_channels:
        return jsonify({'error': f'Invalid channel. Valid options: {valid_channels}'}), 400

    # Get current config to preserve masked values
    current_config = NotificationService.get_config()

    # Handle masked sensitive values
    if channel == 'discord':
        if data.get('webhook_url') and '...' in data.get('webhook_url', ''):
            data['webhook_url'] = current_config.get('discord', {}).get('webhook_url', '')

    elif channel == 'slack':
        if data.get('webhook_url') and '...' in data.get('webhook_url', ''):
            data['webhook_url'] = current_config.get('slack', {}).get('webhook_url', '')

    elif channel == 'telegram':
        if data.get('bot_token') == '***':
            data['bot_token'] = current_config.get('telegram', {}).get('bot_token', '')

    elif channel == 'email':
        if data.get('smtp_password') == '***':
            data['smtp_password'] = current_config.get('email', {}).get('smtp_password', '')

    elif channel == 'generic_webhook':
        if data.get('url') and '...' in data.get('url', ''):
            data['url'] = current_config.get('generic_webhook', {}).get('url', '')

    result = NotificationService.update_channel_config(channel, data)
    return jsonify(result), 200 if result['success'] else 400


@notifications_bp.route('/test/<channel>', methods=['POST'])
@jwt_required()
@admin_required
def test_channel(channel):
    """Send a test notification to a specific channel."""
    valid_channels = ['discord', 'slack', 'telegram', 'email', 'generic_webhook']
    if channel not in valid_channels:
        return jsonify({'error': f'Invalid channel. Valid options: {valid_channels}'}), 400

    result = NotificationService.send_test(channel)
    return jsonify(result), 200 if result['success'] else 400


@notifications_bp.route('/test', methods=['POST'])
@jwt_required()
@admin_required
def test_all_channels():
    """Send a test notification to all enabled channels."""
    test_alerts = [{
        'type': 'test',
        'severity': 'test',
        'message': 'This is a test notification from ServerKit. All channels are working correctly!',
        'value': 'N/A',
        'threshold': 'N/A'
    }]

    # Temporarily enable test severity for all channels
    config = NotificationService.get_config()
    results = {}

    for channel in ['discord', 'slack', 'telegram', 'email', 'generic_webhook']:
        channel_config = config.get(channel, {})
        if channel_config.get('enabled'):
            test_config = {**channel_config, 'notify_on': ['test']}

            if channel == 'discord':
                results[channel] = NotificationService.send_discord(test_alerts, test_config)
            elif channel == 'slack':
                results[channel] = NotificationService.send_slack(test_alerts, test_config)
            elif channel == 'telegram':
                results[channel] = NotificationService.send_telegram(test_alerts, test_config)
            elif channel == 'email':
                results[channel] = NotificationService.send_email(test_alerts, test_config)
            elif channel == 'generic_webhook':
                results[channel] = NotificationService.send_generic_webhook(test_alerts, test_config)

    if not results:
        return jsonify({'success': False, 'error': 'No channels enabled'}), 400

    all_success = all(r.get('success', False) for r in results.values())
    return jsonify({
        'success': all_success,
        'results': results
    }), 200 if all_success else 207  # 207 Multi-Status for partial success


# ==========================================
# USER NOTIFICATION PREFERENCES
# ==========================================

@notifications_bp.route('/preferences', methods=['GET'])
@jwt_required()
def get_user_preferences():
    """Get current user's notification preferences."""
    current_user_id = get_jwt_identity()
    prefs = NotificationPreferences.get_or_create(current_user_id)
    return jsonify(prefs.to_dict()), 200


@notifications_bp.route('/preferences', methods=['PUT'])
@jwt_required()
def update_user_preferences():
    """Update current user's notification preferences."""
    current_user_id = get_jwt_identity()
    data = request.get_json()

    if not data:
        return jsonify({'error': 'No data provided'}), 400

    prefs = NotificationPreferences.get_or_create(current_user_id)

    # Update fields
    if 'enabled' in data:
        prefs.enabled = data['enabled']

    if 'channels' in data:
        valid_channels = ['email', 'discord', 'slack', 'telegram']
        channels = [c for c in data['channels'] if c in valid_channels]
        prefs.set_channels(channels)

    if 'severities' in data:
        valid_severities = ['critical', 'warning', 'info', 'success']
        severities = [s for s in data['severities'] if s in valid_severities]
        prefs.set_severities(severities)

    if 'email' in data:
        prefs.email = data['email'] if data['email'] else None

    if 'discord_webhook' in data:
        # Don't update if it's the masked version
        if data['discord_webhook'] and '...' not in data['discord_webhook']:
            prefs.discord_webhook = data['discord_webhook']
        elif not data['discord_webhook']:
            prefs.discord_webhook = None

    if 'telegram_chat_id' in data:
        prefs.telegram_chat_id = data['telegram_chat_id'] if data['telegram_chat_id'] else None

    if 'categories' in data:
        prefs.set_categories(data['categories'])

    if 'quiet_hours' in data:
        qh = data['quiet_hours']
        if 'enabled' in qh:
            prefs.quiet_hours_enabled = qh['enabled']
        if 'start' in qh:
            prefs.quiet_hours_start = qh['start']
        if 'end' in qh:
            prefs.quiet_hours_end = qh['end']

    try:
        db.session.commit()
        return jsonify({'success': True, 'preferences': prefs.to_dict()}), 200
    except Exception as e:
        db.session.rollback()
        return jsonify({'error': str(e)}), 500


@notifications_bp.route('/preferences/test', methods=['POST'])
@jwt_required()
def test_user_notification():
    """Send a test notification to the current user's configured channels."""
    current_user_id = get_jwt_identity()
    prefs = NotificationPreferences.get_or_create(current_user_id)
    user = User.query.get(current_user_id)

    if not prefs.enabled:
        return jsonify({'error': 'Notifications are disabled for your account'}), 400

    test_alert = {
        'type': 'test',
        'severity': 'test',
        'message': f'This is a test notification for {user.username}. Your personal notification settings are working correctly!',
        'value': 'N/A',
        'threshold': 'N/A'
    }

    results = {}
    channels = prefs.get_channels()
    config = NotificationService.get_config()

    # Send to each enabled channel
    if 'email' in channels:
        email_to_use = prefs.email or user.email
        if email_to_use:
            email_config = {
                **config.get('email', {}),
                'to_emails': [email_to_use],
                'notify_on': ['test']
            }
            results['email'] = NotificationService.send_email([test_alert], email_config)

    if 'discord' in channels and prefs.discord_webhook:
        discord_config = {
            'enabled': True,
            'webhook_url': prefs.discord_webhook,
            'username': 'ServerKit',
            'notify_on': ['test']
        }
        results['discord'] = NotificationService.send_discord([test_alert], discord_config)

    if 'telegram' in channels and prefs.telegram_chat_id:
        telegram_config = {
            **config.get('telegram', {}),
            'chat_id': prefs.telegram_chat_id,
            'notify_on': ['test']
        }
        results['telegram'] = NotificationService.send_telegram([test_alert], telegram_config)

    if 'slack' in channels:
        # Use global slack config for user notifications
        slack_config = {**config.get('slack', {}), 'notify_on': ['test']}
        if slack_config.get('enabled') and slack_config.get('webhook_url'):
            results['slack'] = NotificationService.send_slack([test_alert], slack_config)

    if not results:
        return jsonify({'error': 'No channels configured for notifications'}), 400

    all_success = all(r.get('success', False) for r in results.values())
    return jsonify({
        'success': all_success,
        'results': results
    }), 200 if all_success else 207


# ==========================================
# IN-APP NOTIFICATION CENTER (the bell + history)
# ==========================================

@notifications_bp.route('/inbox', methods=['GET'])
@jwt_required()
def get_inbox():
    """List the current user's in-app notifications, newest first."""
    user_id = int(get_jwt_identity())
    limit = min(request.args.get('limit', 20, type=int), 100)
    offset = request.args.get('offset', 0, type=int)
    unread_only = request.args.get('unread', '').lower() in ('1', 'true', 'yes')
    items = NotificationBusService.inbox(user_id, limit=limit, offset=offset, unread_only=unread_only)
    return jsonify({
        'items': items,
        'unread_count': NotificationBusService.unread_count(user_id),
    }), 200


@notifications_bp.route('/inbox/unread-count', methods=['GET'])
@jwt_required()
def get_unread_count():
    """Lightweight unread count for the bell badge."""
    user_id = int(get_jwt_identity())
    return jsonify({'count': NotificationBusService.unread_count(user_id)}), 200


@notifications_bp.route('/inbox/<int:delivery_id>/read', methods=['POST'])
@jwt_required()
def mark_inbox_read(delivery_id):
    """Mark one in-app notification as read."""
    user_id = int(get_jwt_identity())
    if not NotificationBusService.mark_read(user_id, delivery_id):
        return jsonify({'error': 'Notification not found'}), 404
    return jsonify({'success': True, 'unread_count': NotificationBusService.unread_count(user_id)}), 200


@notifications_bp.route('/inbox/read-all', methods=['POST'])
@jwt_required()
def mark_inbox_all_read():
    """Mark all of the current user's in-app notifications as read."""
    user_id = int(get_jwt_identity())
    updated = NotificationBusService.mark_all_read(user_id)
    return jsonify({'success': True, 'updated': updated, 'unread_count': 0}), 200


# ==========================================
# DELIVERY LOG / OPS (admin)
# ==========================================

@notifications_bp.route('/admin/deliveries', methods=['GET'])
@jwt_required()
@admin_required
def admin_delivery_log():
    """List recent deliveries across all users, with status/channel filters."""
    status = request.args.get('status') or None
    channel = request.args.get('channel') or None
    limit = min(request.args.get('limit', 50, type=int), 200)
    offset = request.args.get('offset', 0, type=int)
    return jsonify({
        'deliveries': NotificationBusService.delivery_log(
            status=status, channel=channel, limit=limit, offset=offset),
        'stats': NotificationBusService.delivery_stats(),
    }), 200


@notifications_bp.route('/admin/deliveries/<int:delivery_id>/retry', methods=['POST'])
@jwt_required()
@admin_required
def admin_retry_delivery(delivery_id):
    """Re-queue a failed delivery for another attempt."""
    result = NotificationBusService.retry_delivery(delivery_id)
    if result is None:
        return jsonify({'error': 'Delivery not found'}), 404
    return jsonify({'success': True, 'delivery': result}), 200


# ==========================================
# EMAIL PROVIDER INTEGRATIONS (admin)
# ==========================================

@notifications_bp.route('/admin/email-providers', methods=['GET'])
@jwt_required()
@admin_required
def list_email_providers():
    """List configured email providers + the catalog of supported types."""
    catalog = {
        key: {'name': spec['name'], 'fields': spec['fields'], 'secrets': spec['secrets']}
        for key, spec in EMAIL_PROVIDERS.items()
    }
    return jsonify({
        'providers': [p.to_dict() for p in EmailProviderService.list_providers()],
        'supported': catalog,
    }), 200


@notifications_bp.route('/admin/email-providers', methods=['POST'])
@jwt_required()
@admin_required
def add_email_provider():
    """Add an email provider, then validate its credentials."""
    data = request.get_json() or {}
    try:
        provider = EmailProviderService.add_provider(data, user_id=int(get_jwt_identity()))
    except ValueError as exc:
        return jsonify({'error': str(exc)}), 400
    test = EmailProviderService.test_provider(provider.id)
    return jsonify({'provider': provider.to_dict(), 'test': test}), 201


@notifications_bp.route('/admin/email-providers/<int:provider_id>/test', methods=['POST'])
@jwt_required()
@admin_required
def test_email_provider(provider_id):
    """Validate a provider's credentials without sending."""
    result = EmailProviderService.test_provider(provider_id)
    return jsonify(result), 200 if result.get('success') else 400


@notifications_bp.route('/admin/email-providers/<int:provider_id>', methods=['PUT'])
@jwt_required()
@admin_required
def update_email_provider(provider_id):
    """Update a provider's usage flags (uses_notifications / uses_relay /
    relay_priority) — §6 unification. uses_relay applies only to SMTP."""
    row = EmailProviderService.update_usage(provider_id, request.get_json() or {})
    if row is None:
        return jsonify({'error': 'Provider not found'}), 404
    return jsonify({'success': True, 'provider': row.to_dict()}), 200


@notifications_bp.route('/admin/email-providers/<int:provider_id>/default', methods=['POST'])
@jwt_required()
@admin_required
def set_default_email_provider(provider_id):
    """Make a provider the default transport."""
    row = EmailProviderService.set_default(provider_id)
    if row is None:
        return jsonify({'error': 'Provider not found'}), 404
    return jsonify({'success': True, 'provider': row.to_dict()}), 200


@notifications_bp.route('/admin/email-providers/<int:provider_id>', methods=['DELETE'])
@jwt_required()
@admin_required
def delete_email_provider(provider_id):
    """Remove an email provider."""
    if not EmailProviderService.delete_provider(provider_id):
        return jsonify({'error': 'Provider not found'}), 404
    return jsonify({'success': True}), 200
