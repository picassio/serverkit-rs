"""Importlib bridge to the ``serverkit-wordpress`` extension backend.

WordPress ships as a bundled, default-installed, uninstallable extension (D4).
Its backend lives in ``builtin-extensions/serverkit-wordpress/backend/`` and is
loaded as the *dashed* package ``app.plugins.serverkit-wordpress``. Because the
slug contains a dash it can never be reached with a normal ``import`` statement
(``import app.plugins.serverkit-wordpress`` is a SyntaxError), so the handful of
core call sites that still reach into the WordPress stack go through this bridge,
which resolves modules with ``importlib`` on a *string* path — dash-safe.

Loadability is uniform across prod / dev / test:

* **Copy-installed** (``app/plugins/serverkit-wordpress`` exists) — the ordinary
  path-based finder resolves it, same as any marketplace plugin.
* **In-place** (dev checkout, the test suite, or a fresh boot *before* the
  flagship seed row is created) — we register the builtin backend directory in
  ``sys.modules`` as the package via ``spec_from_file_location`` with
  ``submodule_search_locations`` pointing at it, so both submodule and relative
  imports inside the extension resolve. **No file copy is required.** This is
  what makes a dashed, default-installed package loadable everywhere.

During the two-speed migration window the bridge also falls back to the legacy
core location (``app.services.<module>``) so callers can be re-pointed *before*
the physical move lands without breaking. Once the files move, the extension
path wins and the fallback is simply never reached.
"""
import importlib

SLUG = 'serverkit-wordpress'
PKG = f'app.plugins.{SLUG}'


def ensure_loadable():
    """Make ``app.plugins.serverkit-wordpress`` importable, in-place if needed.

    Delegates to the canonical in-place loader in ``plugin_service`` (shared with
    the flagship boot path). Idempotent and cheap after the first call. Returns
    ``True`` if the package is (now) importable, ``False`` if the extension isn't
    present on disk at all (in which case callers fall back to the legacy core
    location during the migration window).
    """
    from app.services.plugin_service import _ensure_builtin_backend_importable
    return _ensure_builtin_backend_importable(SLUG)


def load(module_name):
    """Import a module from the extension backend (e.g. ``'wordpress_service'``).

    Falls back to the legacy core location during the migration window.
    """
    if ensure_loadable():
        try:
            return importlib.import_module(f'{PKG}.{module_name}')
        except ImportError:
            pass
    # Two-speed fallback: the file may still live in core (pre-move).
    return importlib.import_module(f'app.services.{module_name}')


def get(module_name, attr):
    """Return an attribute (usually a service class) from an extension module."""
    return getattr(load(module_name), attr)


# ── Convenience accessors for the service classes core code reaches for ──
# All WordPress *models* stay core (app.models.wordpress_site / wordpress_custom_plugin);
# only these services physically move into the extension, so only these need the bridge.

def wordpress_service():
    return get('wordpress_service', 'WordPressService')


def git_wordpress_service():
    return get('git_wordpress_service', 'GitWordPressService')


def wordpress_env_service():
    return get('wordpress_env_service', 'WordPressEnvService')


def wp_update_service():
    return get('wp_update_service', 'WpUpdateService')
