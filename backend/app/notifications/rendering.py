"""Jinja2 rendering for notification emails.

`render_email()` turns a template name + context into a `(subject, html, text)`
bundle. HTML is authored as Jinja templates under ``templates/email/`` that
extend ``base.html``; the plain-text twin is taken from an optional
``<template>.txt`` or generated from the title/body/data as a fallback so every
email always ships a text alternative.

Autoescaping is on for HTML, so caller-supplied data is safe to interpolate.
"""
import logging
import os
from datetime import datetime

from jinja2 import Environment, FileSystemLoader, select_autoescape

from app.notifications.branding import BRAND, SEVERITY_STYLES, style_for

logger = logging.getLogger(__name__)

TEMPLATES_DIR = os.path.join(os.path.dirname(__file__), 'templates')

_env = None


def env():
    """Lazily build the shared Jinja environment."""
    global _env
    if _env is None:
        _env = Environment(
            loader=FileSystemLoader(TEMPLATES_DIR),
            autoescape=select_autoescape(['html', 'xml']),
            trim_blocks=True,
            lstrip_blocks=True,
        )
    return _env


def _hostname():
    try:
        if hasattr(os, 'uname'):
            return os.uname().nodename
        import socket
        return socket.gethostname()
    except Exception:
        return 'serverkit'


def _build_context(subject, severity, data, recipient, urls, hostname):
    palette = style_for(severity)
    data = data or {}
    urls = urls or {}
    now = datetime.now()
    return {
        'brand': BRAND,
        'palette': palette,
        'severity_styles': SEVERITY_STYLES,
        'severity': severity,
        'severity_label': palette['label'],
        'subject': subject,
        'title': subject,
        'preheader': data.get('preheader') or data.get('summary') or subject,
        'data': data,
        'recipient': recipient or {},
        'hostname': hostname or _hostname(),
        'now': now.strftime('%Y-%m-%d %H:%M'),
        'year': now.year,
        'manage_url': urls.get('manage'),
        'unsubscribe_url': urls.get('unsubscribe'),
        'action_url': data.get('action_url') or urls.get('action'),
        'action_label': data.get('action_label') or 'Open ServerKit',
    }


def _render_html(template, ctx):
    name = f'email/{template}.html'
    try:
        return env().get_template(name).render(**ctx)
    except Exception as exc:
        # Never let a template bug swallow a notification — fall back to generic.
        logger.warning('Email template %s failed (%s); using generic', name, exc)
        return env().get_template('email/generic.html').render(**ctx)


def _render_text(template, ctx):
    # Prefer an explicit text twin if the author provided one.
    try:
        return env().get_template(f'email/{template}.txt').render(**ctx)
    except Exception:
        pass
    # Otherwise synthesize a clean text alternative from the context.
    lines = [ctx['subject'], '=' * min(len(ctx['subject']), 60), '']
    summary = ctx['data'].get('summary') or ctx['data'].get('message')
    if summary:
        lines += [str(summary), '']
    for key, value in (ctx['data'] or {}).items():
        if key in ('summary', 'message', 'preheader', 'action_url', 'action_label'):
            continue
        if isinstance(value, (str, int, float, bool)):
            lines.append(f'{key.replace("_", " ").title()}: {value}')
    if ctx.get('action_url'):
        lines += ['', f'{ctx["action_label"]}: {ctx["action_url"]}']
    lines += ['', '-' * 40, f'Sent by {ctx["brand"]["name"]} ({ctx["hostname"]}) at {ctx["now"]}']
    if ctx.get('manage_url'):
        lines.append(f'Manage notifications: {ctx["manage_url"]}')
    return '\n'.join(lines)


def render_email(template, subject, severity='info', data=None,
                 recipient=None, urls=None, hostname=None):
    """Render an email to ``{'subject', 'html', 'text'}``."""
    ctx = _build_context(subject, severity, data, recipient, urls, hostname)
    return {
        'subject': subject,
        'html': _render_html(template or 'generic', ctx),
        'text': _render_text(template or 'generic', ctx),
    }
