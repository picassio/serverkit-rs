"""Channel adapter contract.

Each channel (in-app, email, chat, ...) implements ``ChannelAdapter.deliver``
and returns a ``DeliveryResult``. The consumer maps the result onto the
delivery row + the queue (sent -> complete, failed -> retry/dead-letter,
skipped -> complete without sending).

Adapters render whatever they need themselves (email builds HTML, chat builds
its own payload), so the interface stays a single method.
"""


class DeliveryResult:
    SENT = 'sent'
    FAILED = 'failed'
    SKIPPED = 'skipped'

    def __init__(self, status, message_id=None, error=None):
        self.status = status
        self.message_id = message_id
        self.error = error

    @property
    def ok(self):
        return self.status == self.SENT

    @classmethod
    def sent(cls, message_id=None):
        return cls(cls.SENT, message_id=message_id)

    @classmethod
    def failed(cls, error):
        return cls(cls.FAILED, error=str(error)[:1000] if error else None)

    @classmethod
    def skipped(cls, reason=None):
        return cls(cls.SKIPPED, error=str(reason)[:1000] if reason else None)

    def __repr__(self):
        return f'<DeliveryResult {self.status}{" " + self.error if self.error else ""}>'


class ChannelAdapter:
    """Base class for a notification channel. Subclasses set ``key`` and
    implement ``deliver``."""

    key = None

    def deliver(self, delivery, notification):
        """Transmit ``notification`` to ``delivery.target`` for this channel.

        Returns a DeliveryResult. May raise — the consumer treats an exception
        the same as DeliveryResult.failed (so the queue retries).
        """
        raise NotImplementedError
