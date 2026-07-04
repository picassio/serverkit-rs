"""Event catalog for the Notification Bus.

Maps an ``event_key`` (e.g. ``backup.completed``) to its presentation defaults:
which template renders it, its default severity, its preference category, and a
title template. Callers pass data; the catalog turns ``event_key`` + data into a
title + routing metadata.

Plugins extend the catalog with ``register(...)`` so a plugin event renders
through the same pipeline as a core one.
"""
import logging

logger = logging.getLogger(__name__)

# category is one of: system | security | backups | apps  (matches
# NotificationPreferences.get_categories()). Title is a str.format template
# applied to the event data (missing keys degrade gracefully — see resolve()).
_CATALOG = {
    # --- backups ---
    'backup.completed': {
        'title': 'Backup completed: {app}',
        'template': 'backup_completed',
        'severity': 'success',
        'category': 'backups',
    },
    'backup.failed': {
        'title': 'Backup failed: {app}',
        'template': 'generic',
        'severity': 'critical',
        'category': 'backups',
    },
    'restore.completed': {
        'title': 'Restore completed: {app}',
        'template': 'generic',
        'severity': 'success',
        'category': 'backups',
    },
    'restore.failed': {
        'title': 'Restore failed: {app}',
        'template': 'generic',
        'severity': 'critical',
        'category': 'backups',
    },
    # --- security ---
    'security.alert': {
        'title': 'Security alert: {alert_type}',
        'template': 'security_alert',
        'severity': 'critical',
        'category': 'security',
    },
    # --- apps / system ---
    'app.deployed': {
        'title': 'Deployed: {app}',
        'template': 'generic',
        'severity': 'success',
        'category': 'apps',
    },
    'system.alert': {
        'title': 'System alert on {hostname}',
        'template': 'generic',
        'severity': 'warning',
        'category': 'system',
    },
    'dns.sync_failed': {
        'title': 'DNS sync failed: {record}',
        'template': 'generic',
        'severity': 'warning',
        'category': 'system',
    },
    # Daily registry check found newer versions of installed extensions (#50).
    'extensions.updates_available': {
        'title': 'Extension updates available ({count})',
        'template': 'generic',
        'severity': 'info',
        'category': 'system',
    },
    # Managed-sites publishing readiness — nudged when a site is created but the
    # base-domain / DNS / HTTPS config is only partly set up.
    'sites.publish.no_base_domain': {
        'title': 'Publish your sites at a real domain',
        'template': 'generic',
        'severity': 'warning',
        'category': 'system',
    },
    'sites.publish.http_only': {
        'title': 'Managed sites are served over HTTP',
        'template': 'generic',
        'severity': 'info',
        'category': 'system',
    },
    'sites.publish.no_server_ip': {
        'title': 'Set a server IP so site DNS can auto-create',
        'template': 'generic',
        'severity': 'warning',
        'category': 'system',
    },
    'sites.publish.base_overlaps_panel': {
        'title': 'Site base domain overlaps the panel domain',
        'template': 'generic',
        'severity': 'warning',
        'category': 'system',
    },
    # Multi-alert monitoring digest (used by the legacy send_all path).
    'monitoring.alert': {
        'title': 'ServerKit alert',
        'template': 'monitoring_alert',
        'severity': 'warning',
        'category': 'system',
    },
    # --- account ---
    'user.welcome': {
        'title': 'Welcome to ServerKit',
        'template': 'welcome',
        'severity': 'info',
        'category': 'system',
    },
    'user.invitation': {
        'title': "You've been invited to ServerKit",
        'template': 'invitation',
        'severity': 'info',
        'category': 'system',
    },
}

# Used when an event_key has no catalog entry — still renders, via generic.html.
DEFAULT_ENTRY = {
    'title': 'Notification',
    'template': 'generic',
    'severity': 'info',
    'category': 'system',
}


def register(event_key, title, template='generic', severity='info', category='system'):
    """Register (or override) a catalog event. Safe to call at import time."""
    _CATALOG[event_key] = {
        'title': title,
        'template': template,
        'severity': severity,
        'category': category,
    }


def _safe_format(template_str, data):
    """str.format that won't explode on a missing/extra key."""
    class _Default(dict):
        def __missing__(self, key):
            return '{' + key + '}'
    try:
        return template_str.format_map(_Default(data or {}))
    except Exception:
        return template_str


def resolve(event_key, data=None, severity=None, title=None):
    """Resolve an event into concrete presentation metadata.

    Explicit ``severity``/``title`` args (from notify.send) win over catalog
    defaults. Returns a dict: title, template, severity, category.
    """
    entry = _CATALOG.get(event_key, DEFAULT_ENTRY)
    resolved_title = title or _safe_format(entry['title'], data)
    return {
        'title': resolved_title,
        'template': entry['template'],
        'severity': severity or entry['severity'],
        'category': entry['category'],
    }


def get(event_key):
    """Return the raw catalog entry for an event, or None."""
    return _CATALOG.get(event_key)
