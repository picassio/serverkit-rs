"""Subprocess + privilege helpers for the installer bootstrap.

Stdlib-only on purpose: this code can run *before* the backend venv exists, so
it must not import ``app.utils.system`` (which imports Flask). It mirrors that
module's ``needs_sudo``/``run_privileged`` semantics so the two stay
conceptually unified — a command list is wrapped with ``sudo`` only when the
process isn't already root, isn't on Windows, and ``sudo`` is available.
"""
from __future__ import annotations

import os
import shutil
import subprocess
from typing import List, Sequence


def needs_sudo() -> bool:
    """Return True if commands should be prefixed with ``sudo``."""
    if os.name == "nt":
        return False
    geteuid = getattr(os, "geteuid", None)
    if geteuid is not None and geteuid() == 0:
        return False
    if not shutil.which("sudo"):
        return False
    return True


def with_privilege(cmd: Sequence[str]) -> List[str]:
    """Return *cmd* with ``sudo`` prepended when necessary."""
    parts = list(cmd)
    if parts and needs_sudo() and parts[0] != "sudo":
        return ["sudo"] + parts
    return parts


def run(cmd: Sequence[str], *, dry_run: bool = False, check: bool = False,
        capture: bool = True, **kwargs) -> subprocess.CompletedProcess:
    """Run *cmd* (a list). Under ``dry_run`` nothing executes — a synthetic,
    successful CompletedProcess is returned so callers can treat both paths
    uniformly.
    """
    parts = list(cmd)
    if dry_run:
        return subprocess.CompletedProcess(parts, 0, stdout="", stderr="")
    kwargs.setdefault("text", True)
    if capture:
        kwargs.setdefault("stdout", subprocess.PIPE)
        kwargs.setdefault("stderr", subprocess.PIPE)
    return subprocess.run(parts, check=check, **kwargs)


def command_ok(cmd: Sequence[str]) -> bool:
    """Run *cmd* quietly and return True iff it exits 0. Never raises."""
    try:
        result = subprocess.run(
            list(cmd),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        return result.returncode == 0
    except (OSError, ValueError):
        return False
