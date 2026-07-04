/**
 * serverkit-tools — native pi tools that let the assistant control ServerKit.
 * Each tool calls the local ServerKit REST API with the per-session token the
 * AI route injects (SK_API_URL + SK_API_TOKEN), so the agent acts AS the
 * requesting user: RBAC, validation and protected-resource guards all apply.
 */
import { Type } from "@earendil-works/pi-ai";
import { defineTool, type ExtensionAPI } from "@earendil-works/pi-coding-agent";

const API = process.env.SK_API_URL || "http://127.0.0.1:5000";
const TOKEN = process.env.SK_API_TOKEN || "";

async function call(method: string, path: string, body?: unknown) {
  const res = await fetch(`${API}/api/v1${path}`, {
    method,
    headers: {
      "Content-Type": "application/json",
      ...(TOKEN ? { Authorization: `Bearer ${TOKEN}` } : {}),
    },
    ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
  });
  const text = await res.text();
  let data: unknown;
  try { data = JSON.parse(text); } catch { data = text; }
  return { ok: res.ok, status: res.status, data };
}

function tool(
  name: string,
  label: string,
  description: string,
  parameters: any,
  fn: (p: any) => Promise<any>,
) {
  return defineTool({
    name, label, description, parameters,
    async execute(_id: string, p: any) {
      const r = await fn(p);
      const text = JSON.stringify(r.data ?? r, null, 2);
      return {
        content: [{ type: "text", text: `HTTP ${r.status}\n${text}` }],
        details: r,
        isError: !r.ok,
      };
    },
  });
}

const tools = [
  // ── read ──────────────────────────────────────────────────────────
  tool("sk_system_metrics", "System Metrics",
    "Get host CPU/memory/disk/network/load metrics.",
    Type.Object({}), () => call("GET", "/system/metrics")),
  tool("sk_monitoring_status", "Monitoring Status",
    "Current alert thresholds, active alerts and metric snapshot.",
    Type.Object({}), () => call("GET", "/monitoring/status")),
  tool("sk_docker_ps", "List Containers",
    "List all Docker containers (running + stopped).",
    Type.Object({}), () => call("GET", "/docker/containers?all=true")),
  tool("sk_db_status", "Database Status",
    "MySQL/MariaDB + PostgreSQL install/running status.",
    Type.Object({}), () => call("GET", "/databases/status")),
  tool("sk_nginx_sites", "List nginx Sites",
    "List configured nginx vhosts.",
    Type.Object({}), () => call("GET", "/nginx/sites")),
  tool("sk_magento_stores", "List Magento Stores",
    "List all Magento stores with status, domains, ports and options.",
    Type.Object({}), () => call("GET", "/magento/stores")),
  tool("sk_store_health", "Magento Store Health",
    "Data-plane services, cron backlog and indexer status for a store.",
    Type.Object({ id: Type.Number({ description: "store id" }) }),
    (p) => call("GET", `/magento/stores/${p.id}/health`)),
  tool("sk_store_log", "Magento Provision Log",
    "Tail the provisioning log of a store.",
    Type.Object({ id: Type.Number(), lines: Type.Optional(Type.Number()) }),
    (p) => call("GET", `/magento/stores/${p.id}/log?lines=${p.lines ?? 60}`)),
  tool("sk_templates_list", "List App Templates",
    "List marketplace app templates (optionally filter by category/search).",
    Type.Object({ category: Type.Optional(Type.String()), search: Type.Optional(Type.String()) }),
    (p) => call("GET", `/templates?${new URLSearchParams(
      Object.fromEntries(Object.entries(p).filter(([, v]) => v))).toString()}`)),

  // ── write (RBAC-gated server-side by the caller's token) ───────────
  tool("sk_create_magento_store", "Create Magento Store",
    "Provision a complete Magento store: data-plane containers (MariaDB/OpenSearch/Redis/Mailpit), exact Composer, install, nginx vhost and cron. Optional TLS, headless mode, RabbitMQ, Varnish, run-as user and per-service versions.",
    Type.Object({
      name: Type.String({ description: "lowercase slug, e.g. shop4" }),
      domain: Type.String(),
      magento_version: Type.Optional(Type.String({ description: "e.g. 2.4.8 (default)" })),
      distribution: Type.Optional(Type.String({ description: "mage-os (default) | magento" })),
      ssl: Type.Optional(Type.String({ description: "none | self-signed | letsencrypt" })),
      headless_mode: Type.Optional(Type.String({ description: "none | shared | separate | split" })),
      run_user: Type.Optional(Type.String({ description: "unix user (default www-data)" })),
      use_rabbitmq: Type.Optional(Type.Boolean()),
      use_varnish: Type.Optional(Type.Boolean()),
    }),
    (p) => call("POST", "/magento/stores", p)),
  tool("sk_magento_action", "Magento Quick Action",
    "Run a bin/magento action on a store. Actions: cache-flush, cache-clean, cache-status, reindex, indexer-status, setup-upgrade, di-compile, static-deploy, maintenance-enable, maintenance-disable, maintenance-status, mode-show, mode-developer, mode-production, cron-run.",
    Type.Object({ id: Type.Number(), action: Type.String() }),
    (p) => call("POST", `/magento/stores/${p.id}/actions/${p.action}`)),
  tool("sk_store_backup", "Backup Store Database",
    "Create a gzipped DB backup for a Magento store.",
    Type.Object({ id: Type.Number() }),
    (p) => call("POST", `/magento/stores/${p.id}/backups`)),
  tool("sk_delete_store", "Delete Magento Store",
    "Tear down a store: data-plane containers+volumes, vhost, cron. Set remove_files to also delete the source on disk. Destructive.",
    Type.Object({ id: Type.Number(), remove_files: Type.Optional(Type.Boolean()) }),
    (p) => call("DELETE", `/magento/stores/${p.id}`, { remove_files: !!p.remove_files })),
  tool("sk_container_control", "Control Container",
    "start | stop | restart a Docker container by id or name. ServerKit's own containers are protected.",
    Type.Object({ id: Type.String(), action: Type.String({ description: "start|stop|restart" }) }),
    (p) => call("POST", `/docker/containers/${p.id}/${p.action}`)),
  tool("sk_create_site", "Create Website (nginx vhost)",
    "Create an nginx site. app_type: php | static | docker | python. Provide domains and a root_path (php/static) or port (docker/python).",
    Type.Object({
      name: Type.String(),
      app_type: Type.String(),
      domains: Type.Array(Type.String()),
      root_path: Type.Optional(Type.String()),
      port: Type.Optional(Type.Number()),
      php_version: Type.Optional(Type.String()),
    }),
    (p) => call("POST", "/nginx/sites", p)),
  tool("sk_install_template", "Install App Template",
    "Deploy a marketplace template as a running app (renders compose + docker compose up).",
    Type.Object({
      template_id: Type.String(),
      app_name: Type.String({ description: "lowercase slug, 3+ chars" }),
      variables: Type.Optional(Type.Record(Type.String(), Type.String())),
    }),
    (p) => call("POST", `/templates/${p.template_id}/install`, { app_name: p.app_name, variables: p.variables || {} })),
  tool("sk_cron_add", "Add Cron Job",
    "Add a system cron job (absolute-path command, no shell operators).",
    Type.Object({ schedule: Type.String(), command: Type.String(), name: Type.Optional(Type.String()) }),
    (p) => call("POST", "/cron/jobs", p)),
  tool("sk_nginx_reload", "Reload nginx",
    "Test and reload the nginx configuration.",
    Type.Object({}), () => call("POST", "/nginx/reload")),
];

export default function (pi: ExtensionAPI) {
  for (const t of tools) pi.registerTool(t);
}
