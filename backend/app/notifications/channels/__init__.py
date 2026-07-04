"""Channel adapter registry.

Maps a channel key -> adapter instance. ``register_adapter`` lets a plugin add
a new channel (SMS, web-push, ...) that the consumer can then deliver, exactly
like core channels.
"""
from app.notifications.channels.base import ChannelAdapter, DeliveryResult
from app.notifications.channels.inapp import InAppAdapter
from app.notifications.channels.email import EmailAdapter
from app.notifications.channels.chat import ChatAdapter

# Channels the bus knows how to deliver out of the box.
_REGISTRY = {
    'inapp': InAppAdapter(),
    'email': EmailAdapter(),
    'discord': ChatAdapter('discord'),
    'slack': ChatAdapter('slack'),
    'telegram': ChatAdapter('telegram'),
    'webhook': ChatAdapter('webhook'),
}

# Channels that can be targeted per-user from a user's NotificationPreferences.
# (Slack/generic-webhook are system-wide today, so they're not auto-selected for
# individual recipients — but remain deliverable if a target is supplied.)
PER_USER_CHANNELS = ('inapp', 'email', 'discord', 'telegram')


def register_adapter(adapter):
    """Register (or replace) a channel adapter. ``adapter.key`` is the channel."""
    if not getattr(adapter, 'key', None):
        raise ValueError('adapter must define a non-empty key')
    _REGISTRY[adapter.key] = adapter
    return adapter


def get_adapter(channel):
    """Return the adapter for a channel key, or None if unknown."""
    return _REGISTRY.get(channel)


def known_channels():
    return sorted(_REGISTRY.keys())


__all__ = [
    'ChannelAdapter', 'DeliveryResult',
    'register_adapter', 'get_adapter', 'known_channels',
    'PER_USER_CHANNELS',
]
