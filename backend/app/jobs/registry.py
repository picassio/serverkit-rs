"""In-process registry mapping a job ``kind`` to its handler.

A handler is a callable ``fn(job) -> result`` where ``job`` is the ``Job`` ORM
row. The return value (JSON-serializable, or ``None``) is stored as the job
result. Raising propagates to the consumer, which records the error and lets the
Queue Bus retry / dead-letter.
"""
import logging

logger = logging.getLogger(__name__)

_HANDLERS = {}


def register(kind, handler, replace=False):
    """Register ``handler`` for ``kind``. Existing kinds are kept unless
    ``replace=True`` (so a plugin reload re-registers cleanly)."""
    if not kind or not callable(handler):
        raise ValueError('register(kind, handler): kind required and handler must be callable')
    if kind in _HANDLERS and not replace:
        logger.debug('Job handler for %r already registered; keeping existing', kind)
        return
    _HANDLERS[kind] = handler


def handler(kind, replace=False):
    """Decorator form of :func:`register`."""
    def _decorator(fn):
        register(kind, fn, replace=replace)
        return fn
    return _decorator


def get(kind):
    return _HANDLERS.get(kind)


def is_registered(kind):
    return kind in _HANDLERS


def registered_kinds():
    return sorted(_HANDLERS.keys())


def clear():
    """Test helper — drop all registered handlers."""
    _HANDLERS.clear()
