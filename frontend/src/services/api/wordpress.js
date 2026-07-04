// WordPress standalone endpoints (from api.js, NOT the separate wordpress.js service)

export async function getWordPressStatus() {
    return this.request('/wordpress/standalone/status');
}

export async function getWordPressRequirements() {
    return this.request('/wordpress/standalone/requirements');
}

export async function installWordPress(data) {
    return this.request('/wordpress/standalone/install', {
        method: 'POST',
        body: data
    });
}

export async function uninstallWordPress(removeData = false) {
    return this.request('/wordpress/standalone/uninstall', {
        method: 'POST',
        body: { removeData }
    });
}

export async function startWordPress() {
    return this.request('/wordpress/standalone/start', { method: 'POST' });
}

export async function stopWordPress() {
    return this.request('/wordpress/standalone/stop', { method: 'POST' });
}

export async function restartWordPress() {
    return this.request('/wordpress/standalone/restart', { method: 'POST' });
}
