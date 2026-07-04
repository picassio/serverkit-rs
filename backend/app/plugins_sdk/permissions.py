"""Extension permission model + capability gate (#25).

A manifest's declared ``permissions`` become enforceable capability checks. A
plugin calls ``require(slug, capability)`` (or the SDK re-export
``require_permission``) before doing something privileged; if the plugin did not
declare that permission, it raises ``PermissionDenied``.

This is **in-process, declaration-based** enforcement. Combined with install-time
consent (the install dialog surfaces requested permissions from the manifest) and
the curated registry, it's the accepted risk posture (decision D6). It does NOT
sandbox a plugin that reaches for a raw host import without going through the gate
— true out-of-process isolation is deliberately out of scope (plan #42). The gate
makes honest plugins verifiable and gives the host a single choke point to tighten
later.
"""
import logging

logger = logging.getLogger(__name__)

# Canonical host capabilities a manifest may request. Agent-command permissions
# use the namespaced form ``agent.command:<action>`` and are matched verbatim.
KNOWN_PERMISSIONS = {'docker', 'filesystem', 'shell', 'network', 'db'}


class PermissionDenied(PermissionError):
    """Raised when a plugin uses a capability it did not declare."""


def declared_permissions(slug):
    """The set of permissions the installed plugin `slug` declared (empty if the
    plugin is unknown)."""
    from app.models.plugin import InstalledPlugin
    p = InstalledPlugin.query.filter_by(slug=slug).first()
    if not p:
        return set()
    perms = (p.manifest or {}).get('permissions') or []
    if not isinstance(perms, list):
        return set()
    return {str(x) for x in perms}


def has(slug, capability):
    """True if plugin `slug` declared `capability`."""
    return capability in declared_permissions(slug)


def require(slug, capability):
    """Assert plugin `slug` declared `capability`, else raise PermissionDenied."""
    if not has(slug, capability):
        raise PermissionDenied(
            f"Plugin '{slug}' has not declared the '{capability}' permission. "
            f"Add it to the plugin.json \"permissions\" array."
        )
    return True


def unknown_permissions(permissions):
    """Return declared permissions that aren't recognized host capabilities
    (agent-command permissions are always accepted). Useful for review/consent UI."""
    out = []
    for p in permissions or []:
        p = str(p)
        if p in KNOWN_PERMISSIONS or p.startswith('agent.command:'):
            continue
        out.append(p)
    return out
