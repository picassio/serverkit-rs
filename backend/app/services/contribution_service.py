"""
Contribution Service - Aggregate active-plugin contributions for the UI.

A plugin manifest may declare a `contributions` block with any subset of:

    {
      "nav":            [{ id, label, route, category, icon, ... }],
      "routes":         [{ path, component, layout?, group? }],
      "page_titles":    { "/some-path": "Title" },
      "command_palette":[{ label, path, category, keywords }],
      "widgets":        [{ slot, component }],
      "layouts":        [{ id, component }],
      "tabs":           [{ group, to, label, icon?, end?, order? }]
    }

A `tabs` entry adds a tab to a core-owned TabGroupLayout group; `group` is the
core group id (== the sidebar item id: files | servers | monitoring). Pair it
with a route contribution carrying the same `group` so the page renders INSIDE
that group's TabGroupLayout (shared PageTopbar chrome) instead of as a flat
dashboard route.

The `layout` field on a route may be one of:

    "padded" (default) — render inside DashboardLayout, normal padding
    "full"             — render inside DashboardLayout, no padding
                         (matches /workflow, /files, /docker shape)
    "bare"             — render OUTSIDE DashboardLayout (no sidebar)
                         under PrivateRoute, fullscreen authenticated
    "<custom-id>"      — wrap in a plugin-contributed layout component
                         declared via contributions.layouts

Each entry is tagged with the source plugin's slug so the frontend knows
which plugin a contribution came from (for error attribution + matching
the `component` string against modules discovered by import.meta.glob).

Only plugins with status == 'active' contribute. A plugin without a
contributions block is silently skipped.
"""
from app.models.plugin import InstalledPlugin


def _tag(items, slug):
    out = []
    for item in items or []:
        if not isinstance(item, dict):
            continue
        item = dict(item)
        item['plugin'] = slug
        out.append(item)
    return out


def get_active_contributions():
    plugins = InstalledPlugin.query.filter_by(
        status=InstalledPlugin.STATUS_ACTIVE,
    ).all()

    nav = []
    routes = []
    page_titles = {}
    command_palette = []
    widgets = []
    layouts = []
    tabs = []
    # AI assistant contributions: per-route suggested prompts + custom
    # tool-result renderers, consumed by the core AIAssistant.
    ai = {'suggested_prompts': [], 'tool_renderers': []}

    for p in plugins:
        contrib = (p.manifest or {}).get('contributions') or {}
        if not isinstance(contrib, dict):
            continue

        nav.extend(_tag(contrib.get('nav'), p.slug))
        routes.extend(_tag(contrib.get('routes'), p.slug))
        command_palette.extend(_tag(contrib.get('command_palette'), p.slug))
        widgets.extend(_tag(contrib.get('widgets'), p.slug))
        layouts.extend(_tag(contrib.get('layouts'), p.slug))
        tabs.extend(_tag(contrib.get('tabs'), p.slug))

        ai_contrib = contrib.get('ai')
        if isinstance(ai_contrib, dict):
            ai['suggested_prompts'].extend(_tag(ai_contrib.get('suggested_prompts'), p.slug))
            ai['tool_renderers'].extend(_tag(ai_contrib.get('tool_renderers'), p.slug))

        titles = contrib.get('page_titles')
        if isinstance(titles, dict):
            for path, title in titles.items():
                if isinstance(path, str) and isinstance(title, str):
                    page_titles[path] = title

    return {
        'nav': nav,
        'routes': routes,
        'page_titles': page_titles,
        'command_palette': command_palette,
        'widgets': widgets,
        'layouts': layouts,
        'tabs': tabs,
        'ai': ai,
    }
