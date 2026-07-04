// Secrets manager + inbound webhook gateway API methods.
// NOTE: the ApiService exposes a single `request(path, { method, body })`
// helper (not get/post/patch/delete) — use it like every other api module.

export async function listVaults() {
    return this.request('/vaults');
}

export async function createVault(data) {
    return this.request('/vaults', { method: 'POST', body: data });
}

export async function getVault(id) {
    return this.request(`/vaults/${id}`);
}

export async function updateVault(id, data) {
    return this.request(`/vaults/${id}`, { method: 'PATCH', body: data });
}

export async function deleteVault(id) {
    return this.request(`/vaults/${id}`, { method: 'DELETE' });
}

export async function listSecrets(vaultId) {
    return this.request(`/vaults/${vaultId}/secrets`);
}

export async function createSecret(vaultId, data) {
    return this.request(`/vaults/${vaultId}/secrets`, { method: 'POST', body: data });
}

export async function bulkCreateSecrets(vaultId, secrets) {
    return this.request(`/vaults/${vaultId}/secrets/bulk`, { method: 'POST', body: { secrets } });
}

export async function getSecret(id) {
    return this.request(`/secrets/${id}`);
}

export async function updateSecret(id, data) {
    return this.request(`/secrets/${id}`, { method: 'PATCH', body: data });
}

export async function revealSecret(id) {
    return this.request(`/secrets/${id}/reveal`, { method: 'POST' });
}

export async function deleteSecret(id) {
    return this.request(`/secrets/${id}`, { method: 'DELETE' });
}

export async function listWebhookEndpoints() {
    return this.request('/webhooks/endpoints');
}

export async function createWebhookEndpoint(data) {
    return this.request('/webhooks/endpoints', { method: 'POST', body: data });
}

export async function getWebhookEndpoint(id) {
    return this.request(`/webhooks/endpoints/${id}`);
}

export async function updateWebhookEndpoint(id, data) {
    return this.request(`/webhooks/endpoints/${id}`, { method: 'PATCH', body: data });
}

export async function regenerateWebhookSecret(id) {
    return this.request(`/webhooks/endpoints/${id}/regenerate-secret`, { method: 'POST' });
}

export async function deleteWebhookEndpoint(id) {
    return this.request(`/webhooks/endpoints/${id}`, { method: 'DELETE' });
}

export async function listWebhookDeliveries(endpointId, params = {}) {
    const query = new URLSearchParams(params).toString();
    return this.request(`/webhooks/endpoints/${endpointId}/deliveries${query ? `?${query}` : ''}`);
}

export async function replayWebhookDelivery(deliveryId) {
    return this.request(`/webhooks/deliveries/${deliveryId}/replay`, { method: 'POST' });
}
