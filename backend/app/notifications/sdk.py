"""Plugin/SDK surface for the Notification Bus.

Stable façade so callers (core services and plugins) depend on this rather than
the service internals:

    from app.plugins_sdk import notify
    notify.send('backup.completed', to='admins', data={'app': 'blog'})

Plugins can also extend the bus:

    notify.register_event('myplugin.thing_ready', 'Your thing is ready',
                          template='generic', severity='success', category='apps')
    notify.register_channel(MySmsAdapter())
"""
from app.notifications.service import NotificationBusService


class NotifySdk:
    def send(self, event, to, data=None, channels=None, severity=None, title=None, category=None):
        """Emit a notification (non-blocking). See NotificationBusService.send."""
        return NotificationBusService.send(
            event, to, data=data, channels=channels,
            severity=severity, title=title, category=category,
        )

    def register_event(self, event_key, title, template='generic', severity='info', category='system'):
        """Add/override a catalog event so it renders through the pipeline."""
        from app.notifications import catalog
        return catalog.register(event_key, title, template=template, severity=severity, category=category)

    def register_channel(self, adapter):
        """Register a new channel adapter (must define ``key`` + ``deliver``)."""
        from app.notifications.channels import register_adapter
        return register_adapter(adapter)
