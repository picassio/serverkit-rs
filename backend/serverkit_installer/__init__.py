"""ServerKit installer bootstrap (Phase 5 foundation).

A stdlib-only, import-safe, unit-tested Python package that mirrors the bash
installer's distro/dependency/firewall/service logic. Per the hardening plan
(docs/plans/07_INSTALLER_UPDATER_UNINSTALLER_HARDENING_PLAN.md §8) this is an
*additive foundation*: install.sh remains the canonical installer. The point of
porting the stabilized bash logic here is to get unit-testable orchestration and
a single source of truth for distro detection that the backend can eventually
share.

It is deliberately import-safe with NO dependency on ``app`` (which pulls in
Flask/SQLAlchemy) because the real bootstrap runs *before* the backend venv
exists. It mirrors ``app/utils/system.py``'s ``run_privileged``/sudo contract
rather than importing it.
"""

from . import deps, distro, firewall, service  # noqa: F401

__all__ = ["distro", "deps", "firewall", "service"]
