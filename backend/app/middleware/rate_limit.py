"""Enhanced rate limiting with per-tier limits and response headers."""
from flask import g, request


def get_rate_limit_key():
    """Custom key function for rate limiting based on auth context."""
    # API key auth
    api_key = getattr(g, 'api_key', None)
    if api_key:
        return f'apikey:{api_key.id}'

    # JWT auth - try to get user id
    try:
        from flask_jwt_extended import get_jwt_identity
        user_id = get_jwt_identity()
        if user_id:
            return f'user:{user_id}'
    except Exception:
        pass

    # Fall back to IP
    ip = request.remote_addr
    if request.headers.get('X-Forwarded-For'):
        ip = request.headers.get('X-Forwarded-For').split(',')[0].strip()
    elif request.headers.get('X-Real-IP'):
        ip = request.headers.get('X-Real-IP')
    return f'ip:{ip}'


def get_dynamic_limit():
    """Return rate limit string based on auth context tier."""
    from app.services.settings_service import SettingsService

    # API key tier
    api_key = getattr(g, 'api_key', None)
    if api_key:
        tier = api_key.tier or 'standard'
        setting_key = f'rate_limit_{tier}'
        return SettingsService.get(setting_key, _default_for_tier(tier))

    # Authenticated user
    api_key_user = getattr(g, 'api_key_user', None)
    if api_key_user:
        return SettingsService.get('rate_limit_standard', '100 per minute')

    try:
        from flask_jwt_extended import get_jwt_identity
        user_id = get_jwt_identity()
        if user_id:
            return SettingsService.get('rate_limit_standard', '100 per minute')
    except Exception:
        pass

    # Unauthenticated
    return SettingsService.get('rate_limit_unauthenticated', '30 per minute')


def _default_for_tier(tier):
    """Return default rate limit for a tier."""
    defaults = {
        'standard': '100 per minute',
        'elevated': '500 per minute',
        'unlimited': '5000 per minute',
    }
    return defaults.get(tier, '100 per minute')


def register_rate_limit_headers(app):
    """Register after_request handler to add rate limit headers."""

    @app.after_request
    def add_rate_limit_headers(response):
        # Flask-Limiter adds these headers automatically when configured,
        # but we ensure they are present with standard names
        limit = response.headers.get('X-RateLimit-Limit')
        remaining = response.headers.get('X-RateLimit-Remaining')
        reset = response.headers.get('X-RateLimit-Reset')

        if limit:
            response.headers['X-RateLimit-Limit'] = limit
        if remaining:
            response.headers['X-RateLimit-Remaining'] = remaining
        if reset:
            response.headers['X-RateLimit-Reset'] = reset

        return response
