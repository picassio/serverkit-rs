#!/usr/bin/env python3
"""Extract frontend API route usage from frontend/src/services/api/*.js.

The parser is intentionally dependency-free so it can run in CI before npm/cargo
setup. It recognizes calls like:

    this.request('/path', { method: 'POST' })
    request(`/apps/${id}/start`, { method: 'POST' })
    fetchApi('/path')

Template placeholders are normalized to `{id}` and query strings are stripped.
"""
from __future__ import annotations

import argparse
import json
import re
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Iterable

REQUEST_NAMES = ("request", "fetchApi", "apiRequest", "requestJson")
METHOD_RE = re.compile(r"\bmethod\s*:\s*['\"]([A-Za-z]+)['\"]")


@dataclass(frozen=True)
class RouteUse:
    method: str
    path: str
    family: str
    source: str


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def _strip_template_exprs(s: str) -> str:
    """Replace JS template expressions with a generic path parameter.

    Handles nested braces well enough for route-template expressions. If a
    malformed template expression is encountered, the rest of that expression is
    collapsed to `{id}` rather than leaking JS into the path.
    """
    out: list[str] = []
    i = 0
    while i < len(s):
        if s.startswith("${", i):
            depth = 1
            i += 2
            while i < len(s) and depth:
                if s[i] == "{":
                    depth += 1
                elif s[i] == "}":
                    depth -= 1
                i += 1
            out.append("{id}")
        else:
            out.append(s[i])
            i += 1
    return "".join(out)


def normalize_path(path: str) -> str | None:
    path = _strip_template_exprs(path).strip()
    if path == "/api/v1" or path.startswith("/api/v1/"):
        path = path[len("/api/v1") :] or "/"
    elif path == "/api" or path.startswith("/api/"):
        path = path[len("/api") :] or "/"
    if not path.startswith("/"):
        return None
    path = path.split("?", 1)[0]
    # Drop obvious JS/string-concat leftovers after normalization.
    path = re.sub(r"\s*\+.*$", "", path)
    # Template suffixes such as `/jobs${suffix}` and `/backups/remote${params}`
    # represent query-string builders, not path parameters.
    path = re.sub(r"(?<=/)([A-Za-z0-9_-]+)\{id\}(?=/|$)", r"\1", path)
    path = re.sub(r"\{id\}(?=[A-Za-z_$])", "{id}/", path)
    path = re.sub(r"//+", "/", path)
    if len(path) > 1:
        path = path.rstrip("/")
    return path or None


def _scan_string(text: str, start: int) -> tuple[str, int] | None:
    quote = text[start]
    if quote not in "'\"`":
        return None
    i = start + 1
    buf: list[str] = []
    while i < len(text):
        ch = text[i]
        if ch == "\\":
            if i + 1 < len(text):
                buf.append(text[i + 1])
                i += 2
                continue
        if ch == quote:
            return "".join(buf), i + 1
        buf.append(ch)
        i += 1
    return None


def _looks_like_call(text: str, name_start: int, name: str) -> bool:
    before = text[name_start - 1] if name_start > 0 else ""
    after = text[name_start + len(name)] if name_start + len(name) < len(text) else ""
    if before and (before.isalnum() or before == "_"):
        return False
    if after != "(":
        return False
    return True


def _find_route_calls(text: str) -> Iterable[tuple[str, int, int]]:
    for name in REQUEST_NAMES:
        for m in re.finditer(re.escape(name) + r"\(", text):
            if not _looks_like_call(text, m.start(), name):
                continue
            i = m.end()
            while i < len(text) and text[i].isspace():
                i += 1
            if i >= len(text) or text[i] not in "'\"`":
                continue
            scanned = _scan_string(text, i)
            if not scanned:
                continue
            raw_path, end = scanned
            yield raw_path, m.start(), end


def extract_routes(root: Path | None = None) -> list[RouteUse]:
    repo = root or _repo_root()
    api_dir = repo / "frontend" / "src" / "services" / "api"
    seen: set[tuple[str, str, str]] = set()
    routes: list[RouteUse] = []
    for file in sorted(api_dir.glob("*.js")):
        text = file.read_text()
        for raw_path, _call_start, arg_end in _find_route_calls(text):
            path = normalize_path(raw_path)
            if not path:
                continue
            # Look only at this call's config object; otherwise a later function's
            # `method:` literal can be incorrectly attributed to a GET request.
            call_end = text.find(");", arg_end)
            if call_end == -1 or call_end - arg_end > 900:
                call_end = arg_end + 300
            lookahead = text[arg_end:call_end]
            mm = METHOD_RE.search(lookahead)
            method = mm.group(1).upper() if mm else "GET"
            family = path.split("/", 2)[1] if path.startswith("/") and len(path.split("/")) > 1 else ""
            rel = str(file.relative_to(repo))
            key = (method, path, rel)
            if key in seen:
                continue
            seen.add(key)
            routes.append(RouteUse(method=method, path=path, family=family, source=rel))
    return sorted(routes, key=lambda r: (r.family, r.path, r.method, r.source))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="write JSON inventory")
    parser.add_argument("--summary", action="store_true", help="write family counts")
    args = parser.parse_args()

    routes = extract_routes()
    if args.summary:
        counts: dict[str, int] = {}
        for r in {(r.method, r.path, r.family) for r in routes}:
            counts[r[2]] = counts.get(r[2], 0) + 1
        for family, count in sorted(counts.items()):
            print(f"{family:24} {count}")
        return 0
    if args.json:
        print(json.dumps([asdict(r) for r in routes], indent=2, sort_keys=True))
    else:
        for r in routes:
            print(f"{r.method:6} {r.path:70} {r.source}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
