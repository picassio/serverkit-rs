"""Firewall abstraction — a port of scripts/lib/firewall.sh.

Detection touches the host, but the open/close command *builders* are pure
functions returning lists of argv lists, which makes them trivially unit-tested
and lets ``apply`` run or merely print them under dry-run.
"""
from __future__ import annotations

from typing import List, Optional, Sequence

from ._proc import command_ok, run, with_privilege

BACKENDS = ("firewalld", "ufw", "nftables", "iptables", "none")


def _split(spec: str):
    """'80/tcp' -> ('80', 'tcp'); '80' -> ('80', 'tcp')."""
    if "/" in spec:
        port, proto = spec.split("/", 1)
        return port, proto
    return spec, "tcp"


def detect(override: Optional[str] = None) -> str:
    """Return the active firewall backend. ``override`` forces a result."""
    import shutil

    if override:
        return override
    if shutil.which("firewall-cmd") and command_ok(["firewall-cmd", "--state"]):
        return "firewalld"
    if shutil.which("ufw") and _ufw_active():
        return "ufw"
    if shutil.which("nft") and command_ok(["nft", "list", "ruleset"]):
        return "nftables"
    if shutil.which("iptables"):
        return "iptables"
    return "none"


def _ufw_active() -> bool:
    try:
        import subprocess

        out = subprocess.run(
            ["ufw", "status"], stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, text=True,
        ).stdout or ""
    except (OSError, ValueError):
        return False
    return any(line.strip().lower().startswith("status: active") for line in out.splitlines())


def open_commands(backend: str, ports: Sequence[str]) -> List[List[str]]:
    """Argv lists that open *ports* (each like '80/tcp') for *backend*."""
    cmds: List[List[str]] = []
    for spec in ports:
        port, proto = _split(spec)
        if backend == "firewalld":
            cmds.append(["firewall-cmd", "--permanent", f"--add-port={port}/{proto}"])
        elif backend == "ufw":
            cmds.append(["ufw", "allow", f"{port}/{proto}"])
        elif backend == "iptables":
            cmds.append(["iptables", "-I", "INPUT", "-p", proto, "--dport", port, "-j", "ACCEPT"])
        elif backend == "nftables":
            cmds.append(["nft", "add", "rule", "inet", "filter", "input", proto, "dport", port, "accept"])
    if backend == "firewalld" and cmds:
        cmds.append(["firewall-cmd", "--reload"])
    return cmds


def close_commands(backend: str, ports: Sequence[str]) -> List[List[str]]:
    """Argv lists that close *ports* for *backend*."""
    cmds: List[List[str]] = []
    for spec in ports:
        port, proto = _split(spec)
        if backend == "firewalld":
            cmds.append(["firewall-cmd", "--permanent", f"--remove-port={port}/{proto}"])
        elif backend == "ufw":
            cmds.append(["ufw", "delete", "allow", f"{port}/{proto}"])
        elif backend == "iptables":
            cmds.append(["iptables", "-D", "INPUT", "-p", proto, "--dport", port, "-j", "ACCEPT"])
        # nftables deletion needs a rule handle we can't reliably resolve here.
    if backend == "firewalld" and cmds:
        cmds.append(["firewall-cmd", "--reload"])
    return cmds


def apply(commands: Sequence[Sequence[str]], *, dry_run: bool = False) -> List[str]:
    """Run each command (with privilege). Returns the rendered command strings.

    Best-effort: a failing rule is recorded but does not abort the batch.
    """
    rendered: List[str] = []
    for cmd in commands:
        full = with_privilege(cmd)
        rendered.append(" ".join(full))
        if not dry_run:
            run(full, capture=True)
    return rendered
