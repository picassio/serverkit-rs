// Cloudflare operations — zone settings (SSL/TLS, Speed, Caching, Security) and
// one-click hardening, layered on the existing Cloudflare DNS connection. Zones
// are addressed by their ServerKit DNS zone id (same as the /dns API).

export async function getCloudflareZoneSettings(zoneId) {
    return this.request(`/cloudflare/zones/${zoneId}/settings`);
}

export async function getCloudflareZoneSetting(zoneId, settingId) {
    return this.request(`/cloudflare/zones/${zoneId}/settings/${settingId}`);
}

export async function updateCloudflareZoneSetting(zoneId, settingId, value) {
    return this.request(`/cloudflare/zones/${zoneId}/settings/${settingId}`, {
        method: 'PATCH',
        body: { value },
    });
}

export async function applyCloudflareSettingsPreset(zoneId) {
    return this.request(`/cloudflare/zones/${zoneId}/settings/apply-preset`, {
        method: 'POST',
    });
}

// Purge the zone's Cloudflare cache. payload is { purge_everything: true } or
// { files: [...] } (and on Enterprise, hosts/prefixes/tags).
export async function purgeCloudflareCache(zoneId, payload) {
    return this.request(`/cloudflare/zones/${zoneId}/purge-cache`, {
        method: 'POST',
        body: payload,
    });
}

// WAF custom firewall rules (http_request_firewall_custom phase).
export async function getCloudflareWafRules(zoneId) {
    return this.request(`/cloudflare/zones/${zoneId}/waf/rules`);
}

export async function addCloudflareWafRule(zoneId, rule) {
    return this.request(`/cloudflare/zones/${zoneId}/waf/rules`, {
        method: 'POST',
        body: rule,
    });
}

export async function applyCloudflareWafPreset(zoneId, presetKey, params = {}) {
    return this.request(`/cloudflare/zones/${zoneId}/waf/presets/${presetKey}`, {
        method: 'POST',
        body: { params },
    });
}

export async function updateCloudflareWafRule(zoneId, rulesetId, ruleId, fields) {
    return this.request(`/cloudflare/zones/${zoneId}/waf/rulesets/${rulesetId}/rules/${ruleId}`, {
        method: 'PATCH',
        body: fields,
    });
}

export async function deleteCloudflareWafRule(zoneId, rulesetId, ruleId) {
    return this.request(`/cloudflare/zones/${zoneId}/waf/rulesets/${rulesetId}/rules/${ruleId}`, {
        method: 'DELETE',
    });
}

// Workers (edge hosting). Account is resolved from the zone server-side.
export async function getCloudflareWorkers(zoneId) {
    return this.request(`/cloudflare/zones/${zoneId}/workers`);
}

export async function deployCloudflareWorker(zoneId, payload) {
    return this.request(`/cloudflare/zones/${zoneId}/workers`, {
        method: 'POST',
        body: payload,
    });
}

export async function deleteCloudflareWorker(zoneId, name) {
    return this.request(`/cloudflare/zones/${zoneId}/workers/${encodeURIComponent(name)}`, {
        method: 'DELETE',
    });
}

export async function addCloudflareWorkerRoute(zoneId, pattern, script) {
    return this.request(`/cloudflare/zones/${zoneId}/workers/routes`, {
        method: 'POST',
        body: { pattern, script },
    });
}

export async function deleteCloudflareWorkerRoute(zoneId, routeId) {
    return this.request(`/cloudflare/zones/${zoneId}/workers/routes/${routeId}`, {
        method: 'DELETE',
    });
}

// Cloudflare Tunnels (cloudflared) — expose a local service through the edge.
export async function getCloudflareTunnels(zoneId) {
    return this.request(`/cloudflare/zones/${zoneId}/tunnels`);
}

export async function createCloudflareTunnel(zoneId, name) {
    return this.request(`/cloudflare/zones/${zoneId}/tunnels`, {
        method: 'POST',
        body: { name },
    });
}

export async function deleteCloudflareTunnel(zoneId, tunnelId) {
    return this.request(`/cloudflare/zones/${zoneId}/tunnels/${tunnelId}`, {
        method: 'DELETE',
    });
}

export async function getCloudflareTunnelInstall(zoneId, tunnelId) {
    return this.request(`/cloudflare/zones/${zoneId}/tunnels/${tunnelId}/install`);
}

export async function getCloudflareTunnelHostnames(zoneId, tunnelId) {
    return this.request(`/cloudflare/zones/${zoneId}/tunnels/${tunnelId}/hostnames`);
}

export async function addCloudflareTunnelHostname(zoneId, tunnelId, hostname, service) {
    return this.request(`/cloudflare/zones/${zoneId}/tunnels/${tunnelId}/hostnames`, {
        method: 'POST',
        body: { hostname, service },
    });
}

export async function removeCloudflareTunnelHostname(zoneId, tunnelId, hostname) {
    return this.request(`/cloudflare/zones/${zoneId}/tunnels/${tunnelId}/hostnames`, {
        method: 'DELETE',
        body: { hostname },
    });
}

// Developer platform — R2 buckets, KV namespaces, D1 databases (account-scoped).
export async function getCloudflareStorage(zoneId) {
    return this.request(`/cloudflare/zones/${zoneId}/storage`);
}

export async function createCloudflareR2Bucket(zoneId, name) {
    return this.request(`/cloudflare/zones/${zoneId}/storage/r2`, {
        method: 'POST',
        body: { name },
    });
}

export async function deleteCloudflareR2Bucket(zoneId, name) {
    return this.request(`/cloudflare/zones/${zoneId}/storage/r2/${encodeURIComponent(name)}`, {
        method: 'DELETE',
    });
}

export async function createCloudflareKvNamespace(zoneId, title) {
    return this.request(`/cloudflare/zones/${zoneId}/storage/kv`, {
        method: 'POST',
        body: { title },
    });
}

export async function deleteCloudflareKvNamespace(zoneId, namespaceId) {
    return this.request(`/cloudflare/zones/${zoneId}/storage/kv/${namespaceId}`, {
        method: 'DELETE',
    });
}

export async function createCloudflareD1Database(zoneId, name) {
    return this.request(`/cloudflare/zones/${zoneId}/storage/d1`, {
        method: 'POST',
        body: { name },
    });
}

export async function deleteCloudflareD1Database(zoneId, databaseId) {
    return this.request(`/cloudflare/zones/${zoneId}/storage/d1/${databaseId}`, {
        method: 'DELETE',
    });
}
