"""ServerKit Notification Bus.

A reusable, queue-backed notification layer built on top of the Queue Bus.

Any part of the app (or a plugin) emits an event:

    from app.plugins_sdk import notify
    notify.send('backup.completed', to='admins', data={'app': 'blog', 'size': '2.3 GB'})

The call is non-blocking: it writes a durable ``Notification`` record plus one
``NotificationDelivery`` per (recipient x channel), enqueues them on the Queue
Bus, and returns. A background ``NotificationConsumer`` renders each delivery
and hands it to the matching channel adapter (in-app, email, chat, ...).

Submodules (``models``, ``service``, ``sdk``, ``consumer``, ``channels``,
``rendering``, ``catalog``) are imported lazily by their consumers to keep
model registration (``app.models.__init__``) free of heavy imports.
"""
