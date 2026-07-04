"""Dependency manifest loader.

Reads scripts/deps/manifest.yaml — the declarative per-family package spec — and
resolves the pieces an installer needs (base packages, python spec, node/docker
method, package manager) for a given family. This is the manifest's primary
consumer: the bash installer can't parse YAML before Python exists, so it
mirrors these choices inline; here we read the file directly.
"""
from __future__ import annotations

from pathlib import Path
from typing import Optional

try:
    import yaml
except ImportError:  # pragma: no cover - PyYAML is a hard backend dependency
    yaml = None


def default_manifest_path() -> Path:
    """Repo-relative path to scripts/deps/manifest.yaml.

    deps.py lives at backend/serverkit_installer/deps.py, so the repo root is
    two parents up.
    """
    return Path(__file__).resolve().parents[2] / "scripts" / "deps" / "manifest.yaml"


def load_manifest(path: Optional[str] = None) -> dict:
    """Load and parse the manifest. Raises if PyYAML is unavailable."""
    if yaml is None:
        raise RuntimeError("PyYAML is required to read the dependency manifest")
    manifest_path = Path(path) if path else default_manifest_path()
    with open(manifest_path, encoding="utf-8") as handle:
        return yaml.safe_load(handle) or {}


def base_packages(manifest: dict, family: str) -> list:
    return list((manifest.get("base_packages") or {}).get(family, []))


def python_spec(manifest: dict, family: str) -> dict:
    return dict((manifest.get("python") or {}).get(family, {}))


def node_spec(manifest: dict, family: str) -> dict:
    return dict((manifest.get("node") or {}).get(family, {}))


def docker_spec(manifest: dict, family: str) -> dict:
    return dict((manifest.get("docker") or {}).get(family, {}))


def package_manager(manifest: dict, family: str) -> Optional[str]:
    return (manifest.get("package_manager") or {}).get(family)
