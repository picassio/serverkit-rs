"""Remote extension registry (Phase 2).

The registry is a single curated `index.json` (hosted in a `serverkit-extensions`
repo, submitted via PR). It lists third-party + first-party extensions that aren't
bundled with the panel, so the Marketplace Browse tab has real content without any
DB seeding.

Design rules:
  - Read-only discovery. NOTHING here ever auto-installs; installs are explicit.
  - Offline-tolerant. A failed/absent fetch falls back to the last good cache, then
    to a bundled copy (app/data/registry_index.json) — the Marketplace never blanks.
  - Configurable. SERVERKIT_REGISTRY_URL points at the live index; unset ⇒ bundled.
"""
import json
import logging
import os
import time

import requests

from app.models.plugin import InstalledPlugin

logger = logging.getLogger(__name__)

_BUNDLED_INDEX = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'data', 'registry_index.json'
)

REGISTRY_URL = os.environ.get('SERVERKIT_REGISTRY_URL', '').strip()
try:
    _TTL = int(os.environ.get('SERVERKIT_REGISTRY_TTL', '3600'))
except ValueError:
    _TTL = 3600

# Module-level cache: last successfully-parsed entry list + when we fetched it.
_cache = {'ts': 0.0, 'entries': None, 'source': None}

# Fields we surface for a registry entry, with defaults.
_FIELDS = {
    'slug': '',
    'display_name': '',
    'description': '',
    'version': '0.0.0',
    'category': 'utility',
    'author': '',
    'first_party': False,
    'permissions': [],
    'min_panel_version': None,
    'max_panel_version': None,
    'source': '',
    'sha256': None,
    'homepage': '',
    'icon': None,
    'screenshots': [],
}


def _normalize(raw):
    if not isinstance(raw, dict) or not raw.get('slug'):
        return None
    out = {}
    for key, default in _FIELDS.items():
        out[key] = raw.get(key, default)
    if not isinstance(out['permissions'], list):
        out['permissions'] = []
    if not isinstance(out['screenshots'], list):
        out['screenshots'] = []
    return out


def _read_index_payload(payload):
    exts = payload.get('extensions') if isinstance(payload, dict) else None
    if not isinstance(exts, list):
        return []
    return [e for e in (_normalize(x) for x in exts) if e]


def _load_bundled():
    try:
        with open(_BUNDLED_INDEX, 'r', encoding='utf-8') as f:
            return _read_index_payload(json.load(f))
    except Exception as e:
        logger.warning(f'Could not read bundled registry index: {e}')
        return []


def _fetch_remote():
    if not REGISTRY_URL:
        return None
    resp = requests.get(REGISTRY_URL, timeout=15, headers={
        'Accept': 'application/json',
        'User-Agent': 'ServerKit-Registry/1.0',
    })
    resp.raise_for_status()
    return _read_index_payload(resp.json())


def refresh(force=False):
    """Return the registry entries, refreshing from the remote index when the
    cache is stale. Never raises — falls back to cache, then bundled copy."""
    now = time.time()
    if not force and _cache['entries'] is not None and (now - _cache['ts']) < _TTL:
        return _cache['entries']

    entries = None
    source = None
    try:
        entries = _fetch_remote()
        if entries is not None:
            source = 'remote'
    except Exception as e:
        logger.warning(f'Registry fetch failed ({REGISTRY_URL}): {e}')

    if entries is None:
        # Keep the last good remote cache if we have one; else bundled.
        if _cache['entries'] is not None:
            return _cache['entries']
        entries = _load_bundled()
        source = 'bundled'

    _cache['entries'] = entries
    _cache['ts'] = now
    _cache['source'] = source
    return entries


def list_extensions():
    return refresh()


def get_entry(slug):
    for e in refresh():
        if e['slug'] == slug:
            return e
    return None


def _install_state(slug):
    p = InstalledPlugin.query.filter_by(slug=slug).first()
    if not p:
        return {'installed': False, 'status': 'not_installed', 'installed_version': None}
    return {
        'installed': True,
        'status': p.status,
        'installed_version': p.version,
    }


def to_catalog_dict(entry):
    """Registry entry + live install state, for the Marketplace Browse merge."""
    d = dict(entry)
    d.update(_install_state(entry['slug']))
    d['source_kind'] = 'registry'
    return d


def list_catalog():
    return [to_catalog_dict(e) for e in refresh()]


def registry_source_label():
    return _cache.get('source')
