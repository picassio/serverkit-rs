"""In-app channel.

There is nothing to transmit: the ``NotificationDelivery`` row itself *is* the
in-app notification (the bell reads `channel='inapp'` deliveries for the
recipient, unread = ``read_at IS NULL``). Delivering just marks it sent.
"""
from app.notifications.channels.base import ChannelAdapter, DeliveryResult


class InAppAdapter(ChannelAdapter):
    key = 'inapp'

    def deliver(self, delivery, notification):
        return DeliveryResult.sent()
