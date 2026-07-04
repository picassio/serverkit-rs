# serverkit_installer

Phase 5 foundation from the
[Installer/Updater/Uninstaller Hardening Plan](../../docs/plans/07_INSTALLER_UPDATER_UNINSTALLER_HARDENING_PLAN.md)
(§8). A stdlib-only, import-safe, unit-tested Python port of the bash installer's
detection and orchestration logic.

> **Additive, not a replacement.** `install.sh` is still the canonical installer.
> This package exists so the stabilized multi-distro logic has a single,
> unit-testable home that the backend can eventually share. It is **not** wired
> into `install.sh` yet — per the plan's migration path, the bash scripts stay
> in charge until this is fully validated on real boxes.

## Why it doesn't import `app`

The real bootstrap runs **before** the backend venv exists, so importing
`app.utils.system` (which pulls in Flask/SQLAlchemy) would fail. This package is
deliberately stdlib-only (plus PyYAML for the manifest) and mirrors
`app/utils/system.py`'s `run_privileged`/sudo contract in `_proc.py`.

## Modules

| Module | Mirrors | Responsibility |
|--------|---------|----------------|
| `distro.py` | `install.sh:os_family_from` + `scripts/lib/env.sh` | OS family + container/WSL/systemd detection |
| `deps.py` | `scripts/deps/manifest.yaml` | Load the manifest, resolve packages per family |
| `firewall.py` | `scripts/lib/firewall.sh` | Detect backend, build open/close command lists |
| `service.py` | `scripts/lib/init.sh` | Detect init system, build start/stop/enable commands |
| `main.py` | — | Compose a read-only install **plan** + CLI |
| `_proc.py` | `app/utils/system.py` | sudo-aware subprocess helpers |

The command **builders** are pure functions returning argv lists, so they're
tested without touching a real firewall/init system.

## Usage

```bash
# Read-only: print what an install would do on the detected distro
python -m serverkit_installer plan

# Just the distro detection
python -m serverkit_installer detect --os-release /etc/os-release

# Force backends for a preview on any host
python -m serverkit_installer plan --firewall firewalld --init systemd
```

## Tests

`backend/tests/test_serverkit_installer.py` (46 cases) — family mapping kept in
lockstep with `os_family_from`, manifest resolution for every family, the
firewall/service command builders, environment predicates, and the composed
plan. Pure logic, no host mutation:

```bash
cd backend && pytest tests/test_serverkit_installer.py
```
