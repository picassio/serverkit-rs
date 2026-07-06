// Magento store management (sk-magento — ServerKit-rs fork extension)

export async function getMagentoVersions() {
    return this.request('/magento/versions');
}

export async function getMagentoActions() {
    return this.request('/magento/actions');
}

export async function getMagentoStores() {
    return this.request('/magento/stores');
}

export async function getMagentoStore(id, reveal = false) {
    return this.request(`/magento/stores/${id}${reveal ? '?reveal=true' : ''}`);
}

export async function createMagentoStore(data) {
    return this.request('/magento/stores', { method: 'POST', body: data });
}

export async function deleteMagentoStore(id, removeFiles = false) {
    return this.request(`/magento/stores/${id}`, {
        method: 'DELETE',
        body: { remove_files: removeFiles },
    });
}

export async function getMagentoStoreLog(id, lines = 80) {
    return this.request(`/magento/stores/${id}/log?lines=${lines}`);
}

export async function getMagentoStoreHealth(id) {
    return this.request(`/magento/stores/${id}/health`);
}

export async function runMagentoAction(id, action) {
    return this.request(`/magento/stores/${id}/actions/${action}`, { method: 'POST' });
}

export async function patchMagentoStore(id, data) {
    return this.request(`/magento/stores/${id}`, { method: 'PATCH', body: data });
}

export async function applyMagentoWeb(id) {
    return this.request(`/magento/stores/${id}/apply-web`, { method: 'POST' });
}

export async function magentoFrontendAction(id, action) {
    return this.request(`/magento/stores/${id}/frontend/${action}`, { method: 'POST' });
}

export async function getMagentoRuntime(id) {
    return this.request(`/magento/stores/${id}/runtime`);
}

export async function updateMagentoRuntime(id, data) {
    return this.request(`/magento/stores/${id}/runtime`, { method: 'PATCH', body: data });
}

export async function repairMagentoPermissions(id) {
    return this.request(`/magento/stores/${id}/permissions/repair`, { method: 'POST' });
}

export async function getMagentoVhost(id) {
    return this.request(`/magento/stores/${id}/vhost`);
}

export async function putMagentoVhost(id, content, frontend = false) {
    return this.request(`/magento/stores/${id}/vhost`, {
        method: 'PUT',
        body: { content, frontend },
    });
}

export async function renewMagentoCert(id) {
    return this.request(`/magento/stores/${id}/renew-cert`, { method: 'POST' });
}

export async function listMagentoBackups(id) {
    return this.request(`/magento/stores/${id}/backups`);
}
export async function createMagentoBackup(id) {
    return this.request(`/magento/stores/${id}/backups`, { method: 'POST' });
}
export async function restoreMagentoBackup(id, filename) {
    return this.request(`/magento/stores/${id}/backups/${encodeURIComponent(filename)}/restore`, { method: 'POST' });
}
export async function deleteMagentoBackup(id, filename) {
    return this.request(`/magento/stores/${id}/backups/${encodeURIComponent(filename)}`, { method: 'DELETE' });
}
export async function setMagentoBackupPolicy(id, schedule, retention) {
    return this.request(`/magento/stores/${id}/backups/policy`, { method: 'POST', body: { schedule, retention } });
}
