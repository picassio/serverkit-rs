"""Init-system / service abstraction — a port of scripts/lib/init.sh.

Detection touches the host; the command builders are pure functions returning a
single argv list (or ``None`` when the action is unsupported under the detected
init), which keeps them unit-testable.
"""
from __future__ import annotations

import os
import shutil
from typing import List, Optional

from ._proc import command_ok, run, with_privilege

INITS = ("systemd", "openrc", "runit", "sysvinit", "none")


def detect(override: Optional[str] = None) -> str:
    """Return the detected init system. ``override`` forces a result."""
    if override:
        return override
    if os.path.isdir("/run/systemd/system") or _pid1_is_systemd():
        return "systemd"
    if shutil.which("rc-service"):
        return "openrc"
    if shutil.which("sv") and (os.path.isdir("/etc/runit") or os.path.isdir("/run/runit")):
        return "runit"
    if os.path.isdir("/etc/init.d") or shutil.which("service"):
        return "sysvinit"
    return "none"


def _pid1_is_systemd() -> bool:
    try:
        with open("/proc/1/comm", encoding="utf-8") as handle:
            return handle.read().strip() == "systemd"
    except OSError:
        return False


def start_command(init: str, service: str) -> Optional[List[str]]:
    return {
        "systemd": ["systemctl", "start", service],
        "openrc": ["rc-service", service, "start"],
        "runit": ["sv", "up", service],
        "sysvinit": ["service", service, "start"],
    }.get(init)


def stop_command(init: str, service: str) -> Optional[List[str]]:
    return {
        "systemd": ["systemctl", "stop", service],
        "openrc": ["rc-service", service, "stop"],
        "runit": ["sv", "down", service],
        "sysvinit": ["service", service, "stop"],
    }.get(init)


def enable_command(init: str, service: str) -> Optional[List[str]]:
    if init == "systemd":
        return ["systemctl", "enable", service]
    if init == "openrc":
        return ["rc-update", "add", service]
    if init == "sysvinit":
        if shutil.which("update-rc.d"):
            return ["update-rc.d", service, "enable"]
        if shutil.which("chkconfig"):
            return ["chkconfig", service, "on"]
        return ["update-rc.d", service, "enable"]
    # runit enable is symlink-based; 'none' is unsupported.
    return None


def reload_command(init: str, service: Optional[str] = None) -> Optional[List[str]]:
    if init == "systemd":
        return ["systemctl", "daemon-reload"] if service is None else ["systemctl", "reload", service]
    if service is None:
        return None
    return {
        "openrc": ["rc-service", service, "reload"],
        "sysvinit": ["service", service, "reload"],
    }.get(init)


def is_active(init: str, service: str) -> bool:
    """Return True when *service* is currently running under *init*."""
    if init == "systemd":
        return command_ok(["systemctl", "is-active", "--quiet", service])
    if init == "openrc":
        return command_ok(["rc-service", service, "status"])
    if init == "runit":
        return command_ok(["sv", "status", service])
    if init == "sysvinit":
        return command_ok(["service", service, "status"])
    return False


def apply(command: Optional[List[str]], *, dry_run: bool = False) -> Optional[str]:
    """Run a single service command (with privilege). Returns the rendered
    string, or ``None`` when the action is unsupported under this init.
    """
    if command is None:
        return None
    full = with_privilege(command)
    if not dry_run:
        run(full, capture=True)
    return " ".join(full)
