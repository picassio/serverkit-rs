"""Proving tests for the expanded service-template catalog (SERVICES_ROADMAP).

Universal contract (every bundled template): parses + passes the real
``TemplateService.validate_template``, has an ``id`` matching its filename, and
ships some icon. The roadmap templates added by this work additionally use an
inline base64 icon (offline-safe), are brand-neutral, and use the list-shaped
variables form. The community-repo index generator (#28) must cover them all.
"""
import os

import pytest

from app.services.template_service import TemplateService

TEMPLATES_DIR = TemplateService.LOCAL_TEMPLATES_DIR

# Templates authored/converted for the services roadmap (Phases 0-3).
ROADMAP_IDS = sorted({
    "prompture-hub", "ollama-webui", "qdrant", "chroma", "litellm",
    "flowise", "langflow", "anythingllm", "librechat",
    "audiobookshelf", "calibre-web", "sonarr", "radarr", "prowlarr",
    "qbittorrent", "jellyseerr", "freshrss", "miniflux", "paperless-ngx",
    "stirling-pdf", "actualbudget", "firefly-iii", "memos", "vikunja", "plane",
    "authentik", "beszel", "signoz", "meilisearch", "typesense", "searxng",
    "gotify", "ntfy", "wg-easy", "pihole",
    "chatwoot", "documenso", "metabase", "posthog", "nodebb", "linkding",
    "karakeep",
})


def _template_files():
    return sorted(
        f for f in os.listdir(TEMPLATES_DIR) if f.endswith((".yaml", ".yml"))
    )


@pytest.mark.parametrize("filename", _template_files())
def test_template_parses_and_validates(filename):
    """Every bundled template parses, validates, has a matching id, and an icon."""
    path = os.path.join(TEMPLATES_DIR, filename)
    result = TemplateService.parse_template(path)
    assert result.get("success"), f"{filename}: {result.get('errors') or result.get('error')}"

    tmpl = result["template"]
    stem = filename.rsplit(".", 1)[0]
    assert tmpl.get("id") == stem, f"{filename}: id '{tmpl.get('id')}' != stem '{stem}'"
    assert tmpl.get("icon"), f"{filename}: no icon"


@pytest.mark.parametrize("tid", ROADMAP_IDS)
def test_roadmap_template_conventions(tid):
    """Roadmap templates use an inline base64 icon, list-shaped vars, brand-neutral."""
    path = os.path.join(TEMPLATES_DIR, f"{tid}.yaml")
    assert os.path.exists(path), f"{tid}: template file missing"

    result = TemplateService.parse_template(path)
    assert result.get("success"), f"{tid}: {result.get('errors') or result.get('error')}"
    tmpl = result["template"]

    assert str(tmpl.get("icon", "")).startswith("data:image/svg+xml;base64,"), f"{tid}: not an inline icon"

    variables = tmpl.get("variables", [])
    assert isinstance(variables, list), f"{tid}: variables should be a list"
    assert all(isinstance(v, dict) and "name" in v for v in variables), f"{tid}: bad variable"

    with open(path, encoding="utf-8") as fh:
        assert "coolify" not in fh.read().lower(), f"{tid}: templates must be brand-neutral"


def test_repo_index_covers_all_templates():
    idx = TemplateService.build_repo_index()
    assert idx["count"] == len(_template_files())
    assert idx["schema_version"] == TemplateService.SCHEMA_VERSION
    for t in idx["templates"]:
        assert t["id"] and t["name"] and t["version"], t
        assert t.get("icon"), t


def test_catalog_contains_roadmap_services():
    ids = {t["id"] for t in TemplateService.build_repo_index()["templates"]}
    missing = [e for e in ROADMAP_IDS if e not in ids]
    assert not missing, f"missing from catalog: {missing}"
