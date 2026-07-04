"""Core-module toggles.

Some heavy verticals (Email, WordPress) haven't been extracted into extensions
yet, but an operator who doesn't use them still wants a smaller panel. A module
toggle hides the vertical's nav + routes and 503s its API — the same "disabled"
mechanism the plugin status guard uses — without uninstalling anything. The
toggle state later becomes the auto-install signal when the vertical is actually
extracted (plan #34).

Flagships (WordPress) ship enabled; disabling is opt-in. Nothing here deletes
data — flipping the toggle back restores the module instantly.
"""
import logging

logger = logging.getLogger(__name__)

# name -> presentation metadata. The settings key is module_<name>_enabled.
#
# Email and WordPress used to be module toggles here; both are now bundled
# extensions (serverkit-email #32, serverkit-wordpress #38) — install/uninstall
# them from the Marketplace instead, and the plugin status guard gates their APIs.
# The map is intentionally empty now; the machinery below stays so future
# core-vertical toggles can be added without re-plumbing.
MODULES = {}


def _setting_key(name):
    return f'module_{name}_enabled'


def is_module_enabled(name):
    """True unless an admin explicitly turned the module off. Fail open: any
    error reading settings leaves the module enabled (never silently hide a
    core feature because of a settings glitch)."""
    if name not in MODULES:
        return True
    try:
        from app.services.settings_service import SettingsService
        val = SettingsService.get(_setting_key(name), True)
    except Exception:
        return True
    if isinstance(val, bool):
        return val
    return str(val).strip().lower() not in ('0', 'false', 'no', 'off', '')


def set_module_enabled(name, enabled, user_id=None):
    if name not in MODULES:
        raise ValueError(f'Unknown module: {name}')
    from app.services.settings_service import SettingsService
    SettingsService.set(_setting_key(name), bool(enabled), user_id=user_id)
    return is_module_enabled(name)


def list_modules():
    return [
        {
            'name': name,
            'label': meta['label'],
            'description': meta['description'],
            'enabled': is_module_enabled(name),
        }
        for name, meta in MODULES.items()
    ]


def attach_module_guard(bp, name):
    """Attach a before_request that 503s a blueprint when its module is off.

    Mirrors plugin_service._attach_status_guard so a disabled module's API
    behaves exactly like a disabled plugin's.

    Idempotent: core blueprints are module-level singletons reused across app
    instances (e.g. every test builds a fresh app), and Flask forbids calling
    the `before_request` setup method after a blueprint has been registered
    once. We attach the handler a single time — it lives on the blueprint and
    applies to every app it's registered on — and read the toggle fresh on each
    request, so re-creating the app never re-attaches.
    """
    if getattr(bp, '_module_guard_attached', False):
        return

    def _check():
        if not is_module_enabled(name):
            from flask import jsonify
            return jsonify({
                'error': f"The {MODULES[name]['label']} module is disabled",
                'module': name,
            }), 503
        return None

    bp.before_request(_check)
    bp._module_guard_attached = True
