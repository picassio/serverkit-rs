"""Socket.IO extension point for plugins (#26).

A plugin can own a real-time namespace at ``/ext/<slug>``, guarded by plugin
status exactly like the HTTP blueprint guard: when the plugin isn't active, new
connections to its namespace are refused. This lets real-time features (log tails,
live streams, the serverkit-gui screenshot case) be plugins.

Manifest surface::

    "socket_entry": "sockets:register"      # app.plugins.<slug>.sockets:register

The referenced function returns a mapping of event name → handler::

    def register():
        def on_connect():
            ...
        def on_subscribe(data):
            ...
        return {"connect": on_connect, "subscribe": on_subscribe}

Each handler is wrapped so it only runs while the plugin is active; a ``connect``
to a disabled plugin's namespace is disconnected immediately.
"""
import functools
import logging

logger = logging.getLogger(__name__)


def _guard(slug, fn):
    @functools.wraps(fn)
    def wrapper(*args, **kwargs):
        from app.models.plugin import InstalledPlugin
        p = InstalledPlugin.query.filter_by(slug=slug).first()
        if not p or p.status != InstalledPlugin.STATUS_ACTIVE:
            try:
                from flask_socketio import disconnect
                disconnect()
            except Exception:
                pass
            return None
        return fn(*args, **kwargs)
    return wrapper


def register_namespace(slug, handlers):
    """Register ``handlers`` (event → fn) on the plugin's ``/ext/<slug>``
    namespace, each status-guarded. Best-effort; returns the namespace string or
    None if Socket.IO isn't available (e.g. a worker without it)."""
    if not handlers or not isinstance(handlers, dict):
        return None
    try:
        from app import get_socketio
        io = get_socketio()
    except Exception:
        io = None
    if io is None:
        logger.info(f'Socket.IO not available; skipping namespace for {slug}')
        return None

    namespace = f'/ext/{slug}'
    for event, fn in handlers.items():
        if not callable(fn):
            continue
        try:
            io.on_event(event, _guard(slug, fn), namespace=namespace)
        except Exception as e:
            logger.warning(f"Socket handler '{event}' for {slug} failed to register: {e}")
    logger.info(f'Registered Socket.IO namespace {namespace} for {slug}')
    return namespace


def register_from_manifest(plugin, manifest):
    """Resolve a plugin's ``socket_entry`` and register the namespace it returns."""
    target = (manifest or {}).get('socket_entry')
    if not target or ':' not in str(target):
        return None
    import importlib
    module_name, func_name = str(target).split(':', 1)
    try:
        mod = importlib.import_module(f'app.plugins.{plugin.slug}.{module_name}')
        fn = getattr(mod, func_name, None)
        if not callable(fn):
            return None
        handlers = fn()
    except Exception as e:
        logger.warning(f'socket_entry for {plugin.slug} failed: {e}')
        return None
    return register_namespace(plugin.slug, handlers)
