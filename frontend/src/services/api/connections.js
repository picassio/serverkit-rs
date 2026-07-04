// Domain-registrar connections — the portfolio + expiry surface that powers the
// Connections → Registrars cards and the Domains-page portfolio.

// Unified, read-only list of every connected external account (source, DNS,
// infra, registrar, storage) — the single "what's connected" source of truth.
export async function getAllConnections() {
    return this.request('/connections');
}

export async function getRegistrarConnections() {
    return this.request('/registrars/connections');
}

export async function addRegistrarConnection(data) {
    return this.request('/registrars/connections', { method: 'POST', body: data });
}

export async function deleteRegistrarConnection(id) {
    return this.request(`/registrars/connections/${id}`, { method: 'DELETE' });
}

export async function testRegistrarConnection(id) {
    return this.request(`/registrars/connections/${id}/test`, { method: 'POST' });
}

export async function getRegistrarDomains() {
    return this.request('/registrars/domains');
}

export async function syncRegistrarDomains() {
    return this.request('/registrars/sync', { method: 'POST' });
}

// Container registries — stored credentials for pulling private images. Listing
// is available to any authenticated user (the app-create picker needs it);
// mutations are admin-only server-side. Secrets are never returned (has_secret).
export async function getContainerRegistries() {
    return this.request('/connections/registries');
}

export async function addContainerRegistry(data) {
    return this.request('/connections/registries', { method: 'POST', body: data });
}

export async function updateContainerRegistry(id, data) {
    return this.request(`/connections/registries/${id}`, { method: 'PUT', body: data });
}

export async function deleteContainerRegistry(id) {
    return this.request(`/connections/registries/${id}`, { method: 'DELETE' });
}

export async function testContainerRegistry(id) {
    return this.request(`/connections/registries/${id}/test`, { method: 'POST' });
}
