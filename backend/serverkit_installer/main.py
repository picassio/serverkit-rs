"""Installer bootstrap orchestration (additive foundation).

The canonical installer is still ``install.sh``. This module composes the
detection + manifest + firewall + service pieces into a single "plan" — what an
install *would* do on the detected distro — and prints it. It performs no host
mutation on its own (the ``plan`` command is read-only), so it is safe to run
anywhere, including in CI and on a dev box.

Usage:
    python -m serverkit_installer plan
    python -m serverkit_installer detect --os-release /etc/os-release
"""
from __future__ import annotations

import argparse
import json
from typing import Optional

from . import deps, distro, firewall, service

PANEL_PORTS = ["80/tcp", "443/tcp"]
BACKEND_SERVICE = "serverkit"


def build_plan(
    *,
    os_release_path: str = "/etc/os-release",
    manifest_path: Optional[str] = None,
    firewall_backend: Optional[str] = None,
    init_system: Optional[str] = None,
) -> dict:
    """Return a read-only description of the install actions for this host.

    Every field is derived without mutating the system. ``firewall_backend`` and
    ``init_system`` can be forced (used by tests and dry-run previews).
    """
    box = distro.detect(os_release_path)

    plan: dict = {
        "distro": {
            "id": box.id,
            "id_like": box.id_like,
            "pretty_name": box.pretty_name,
            "family": box.family,
            "supported": box.supported,
        },
        "environment": {
            "container": distro.is_container(),
            "wsl": distro.is_wsl(),
            "systemd": distro.has_systemd(),
        },
    }

    # Packages from the manifest (best-effort: an unknown family or unreadable
    # manifest just yields empty lists rather than blowing up the preview).
    try:
        manifest = deps.load_manifest(manifest_path)
        plan["packages"] = {
            "package_manager": deps.package_manager(manifest, box.family),
            "base": deps.base_packages(manifest, box.family),
            "python": deps.python_spec(manifest, box.family),
            "node": deps.node_spec(manifest, box.family),
            "docker": deps.docker_spec(manifest, box.family),
        }
    except (RuntimeError, OSError) as exc:
        plan["packages"] = {"error": str(exc)}

    fw_backend = firewall.detect(firewall_backend)
    plan["firewall"] = {
        "backend": fw_backend,
        "open": [" ".join(c) for c in firewall.open_commands(fw_backend, PANEL_PORTS)],
    }

    init = service.detect(init_system)
    plan["service"] = {
        "init": init,
        "enable": (lambda c: " ".join(c) if c else None)(service.enable_command(init, BACKEND_SERVICE)),
        "start": (lambda c: " ".join(c) if c else None)(service.start_command(init, BACKEND_SERVICE)),
    }
    return plan


def main(argv: Optional[list] = None) -> int:
    parser = argparse.ArgumentParser(prog="serverkit_installer", description=__doc__)
    parser.add_argument("command", nargs="?", default="plan", choices=["plan", "detect"])
    parser.add_argument("--os-release", default="/etc/os-release")
    parser.add_argument("--manifest", default=None)
    parser.add_argument("--firewall", default=None, help="force a firewall backend")
    parser.add_argument("--init", default=None, help="force an init system")
    parser.add_argument("--json", action="store_true", help="emit JSON")
    args = parser.parse_args(argv)

    if args.command == "detect":
        box = distro.detect(args.os_release)
        if args.json:
            print(json.dumps(box.__dict__))
        else:
            print(f"{box.pretty_name or box.id or 'unknown'} -> family={box.family} "
                  f"(supported={box.supported})")
        return 0

    plan = build_plan(
        os_release_path=args.os_release,
        manifest_path=args.manifest,
        firewall_backend=args.firewall,
        init_system=args.init,
    )
    print(json.dumps(plan, indent=2))
    return 0
