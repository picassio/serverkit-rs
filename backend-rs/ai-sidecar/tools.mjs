// ServerKit native tools for the pi SDK sidecar. Each tool calls back into the
// ServerKit REST API with the per-request token, so the agent acts AS the
// requesting user (RBAC enforced server-side). `ctx` is mutable and refreshed
// on every request so a long-lived session always uses the caller's token.
//
// Parameters use plain JSON Schema (defineTool accepts it) so the sidecar has
// no dependency beyond @earendil-works/pi-coding-agent itself.
import { defineTool } from "@earendil-works/pi-coding-agent";

// tiny JSON-Schema helpers
const S = {
  obj: (props = {}, required = []) => ({ type: "object", properties: props, required }),
  str: (description) => ({ type: "string", ...(description ? { description } : {}) }),
  num: (description) => ({ type: "number", ...(description ? { description } : {}) }),
  bool: (description) => ({ type: "boolean", ...(description ? { description } : {}) }),
  arr: (items) => ({ type: "array", items }),
};

export function makeTools(ctx) {
  async function call(method, path, body) {
    const res = await fetch(`${ctx.apiUrl}/api/v1${path}`, {
      method,
      headers: {
        "Content-Type": "application/json",
        ...(ctx.apiToken ? { Authorization: `Bearer ${ctx.apiToken}` } : {}),
      },
      ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
    });
    const text = await res.text();
    let data;
    try { data = JSON.parse(text); } catch { data = text; }
    return { ok: res.ok, status: res.status, data };
  }

  const tool = (name, label, description, parameters, fn) =>
    defineTool({
      name, label, description, parameters,
      execute: async (_id, p) => {
        const r = await fn(p);
        return {
          content: [{ type: "text", text: `HTTP ${r.status}\n${JSON.stringify(r.data ?? r, null, 2)}` }],
          details: r,
          isError: !r.ok,
        };
      },
    });

  return [
    // ── read ──────────────────────────────────────────────────────────
    tool("sk_system_metrics", "System Metrics", "Host CPU/memory/disk/network/load metrics.",
      S.obj(), () => call("GET", "/system/metrics")),
    tool("sk_monitoring_status", "Monitoring Status", "Alert thresholds, active alerts and metric snapshot.",
      S.obj(), () => call("GET", "/monitoring/status")),
    tool("sk_docker_ps", "List Containers", "All Docker containers (running + stopped).",
      S.obj(), () => call("GET", "/docker/containers?all=true")),
    tool("sk_db_status", "Database Status", "MySQL/MariaDB + PostgreSQL install/running status.",
      S.obj(), () => call("GET", "/databases/status")),
    tool("sk_nginx_sites", "List nginx Sites", "Configured nginx vhosts.",
      S.obj(), () => call("GET", "/nginx/sites")),
    tool("sk_magento_stores", "List Magento Stores", "All Magento stores with status, domains, ports, options.",
      S.obj(), () => call("GET", "/magento/stores")),
    tool("sk_store_health", "Magento Store Health", "Data services, cron backlog and indexer status for a store.",
      S.obj({ id: S.num("store id") }, ["id"]), (p) => call("GET", `/magento/stores/${p.id}/health`)),
    tool("sk_store_log", "Magento Provision Log", "Tail the provisioning log of a store.",
      S.obj({ id: S.num(), lines: S.num() }, ["id"]), (p) => call("GET", `/magento/stores/${p.id}/log?lines=${p.lines ?? 60}`)),
    tool("sk_templates_list", "List App Templates", "Marketplace app templates.",
      S.obj({ category: S.str(), search: S.str() }),
      (p) => call("GET", `/templates?${new URLSearchParams(Object.fromEntries(Object.entries(p).filter(([, v]) => v))).toString()}`)),

    // ── write (RBAC-gated by the caller's token) ──────────────────────
    tool("sk_create_magento_store", "Create Magento Store",
      "Provision a full Magento store: data-plane containers, Composer, install, nginx vhost, cron. Optional TLS, headless mode, RabbitMQ, Varnish, run-as user.",
      S.obj({
        name: S.str("lowercase slug, e.g. shop4"), domain: S.str(),
        magento_version: S.str("e.g. 2.4.8"), distribution: S.str("mage-os (default) | magento"),
        ssl: S.str("none | self-signed | letsencrypt"),
        headless_mode: S.str("none | shared | separate | split"),
        run_user: S.str("unix user (default www-data)"),
        use_rabbitmq: S.bool(), use_varnish: S.bool(),
      }, ["name", "domain"]), (p) => call("POST", "/magento/stores", p)),
    tool("sk_magento_action", "Magento Quick Action",
      "Run a bin/magento action: cache-flush, cache-clean, reindex, indexer-status, setup-upgrade, di-compile, static-deploy, maintenance-enable/disable, mode-developer/production, cron-run.",
      S.obj({ id: S.num(), action: S.str() }, ["id", "action"]),
      (p) => call("POST", `/magento/stores/${p.id}/actions/${p.action}`)),
    tool("sk_store_backup", "Backup Store Database", "Gzipped DB backup for a Magento store.",
      S.obj({ id: S.num() }, ["id"]), (p) => call("POST", `/magento/stores/${p.id}/backups`)),
    tool("sk_delete_store", "Delete Magento Store",
      "Tear down a store (containers+volumes, vhost, cron). remove_files also deletes source on disk. Destructive.",
      S.obj({ id: S.num(), remove_files: S.bool() }, ["id"]),
      (p) => call("DELETE", `/magento/stores/${p.id}`, { remove_files: !!p.remove_files })),
    tool("sk_container_control", "Control Container", "start | stop | restart a container by id/name. ServerKit's own are protected.",
      S.obj({ id: S.str(), action: S.str("start|stop|restart") }, ["id", "action"]),
      (p) => call("POST", `/docker/containers/${p.id}/${p.action}`)),
    tool("sk_create_site", "Create Website (nginx vhost)",
      "Create an nginx site. app_type: php | static | docker | python. Provide domains + root_path (php/static) or port (docker/python).",
      S.obj({
        name: S.str(), app_type: S.str(), domains: S.arr(S.str()),
        root_path: S.str(), port: S.num(), php_version: S.str(),
      }, ["name", "app_type", "domains"]), (p) => call("POST", "/nginx/sites", p)),
    tool("sk_install_template", "Install App Template", "Deploy a marketplace template as a running app.",
      S.obj({ template_id: S.str(), app_name: S.str(), variables: { type: "object" } }, ["template_id", "app_name"]),
      (p) => call("POST", `/templates/${p.template_id}/install`, { app_name: p.app_name, variables: p.variables || {} })),
    tool("sk_cron_add", "Add Cron Job", "Add a system cron job (absolute-path command, no shell operators).",
      S.obj({ schedule: S.str(), command: S.str(), name: S.str() }, ["schedule", "command"]),
      (p) => call("POST", "/cron/jobs", p)),
    tool("sk_nginx_reload", "Reload nginx", "Test and reload the nginx configuration.",
      S.obj(), () => call("POST", "/nginx/reload")),
  ];
}
