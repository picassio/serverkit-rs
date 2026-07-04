// Node.js runtime endpoints. These are intentionally tracked by the route
// ledger so /node is a first-class Rust-backed route family.

export async function getNodeVersions() {
    return this.request('/node/versions');
}

export async function listNodeApps() {
    return this.request('/node/apps');
}

export async function createNodeApp(data) {
    return this.request('/node/apps', {
        method: 'POST',
        body: data
    });
}

export async function getNodeApp(appId) {
    return this.request(`/node/apps/${appId}`);
}

export async function deleteNodeApp(appId, removeFiles = false) {
    return this.request(`/node/apps/${appId}`, {
        method: 'DELETE',
        body: { remove_files: removeFiles }
    });
}

export async function getNodePackages(appId) {
    return this.request(`/node/apps/${appId}/packages`);
}

export async function installNodePackages(appId, packages) {
    return this.request(`/node/apps/${appId}/packages`, {
        method: 'POST',
        body: { packages }
    });
}

export async function getNodeEnvVars(appId) {
    return this.request(`/node/apps/${appId}/env`);
}

export async function setNodeEnvVars(appId, envVars) {
    return this.request(`/node/apps/${appId}/env`, {
        method: 'PUT',
        body: { env_vars: envVars }
    });
}

export async function deleteNodeEnvVar(appId, key) {
    return this.request(`/node/apps/${appId}/env/${key}`, { method: 'DELETE' });
}

export async function setNodeStartCommand(appId, startCommand) {
    return this.request(`/node/apps/${appId}/start-command`, {
        method: 'PUT',
        body: { start_command: startCommand }
    });
}

export async function startNodeApp(appId) {
    return this.request(`/node/apps/${appId}/start`, { method: 'POST' });
}

export async function stopNodeApp(appId) {
    return this.request(`/node/apps/${appId}/stop`, { method: 'POST' });
}

export async function restartNodeApp(appId) {
    return this.request(`/node/apps/${appId}/restart`, { method: 'POST' });
}

export async function getNodeAppStatus(appId) {
    return this.request(`/node/apps/${appId}/status`);
}

export async function runNodeCommand(appId, command) {
    return this.request(`/node/apps/${appId}/run`, {
        method: 'POST',
        body: { command }
    });
}
