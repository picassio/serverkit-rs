import { createContext, useContext } from 'react';

// Server context for Docker operations
export const ServerContext = createContext({ serverId: 'local', serverName: 'Local' });
export const useServer = () => useContext(ServerContext);

export const VALID_TABS = ['containers', 'compose', 'images', 'volumes', 'networks'];
export const LOCAL_DOCKER_TARGET = { id: 'local', name: 'Local (this server)', status: 'online', is_local: true };

export const unwrapRemoteData = (response) => {
    if (response?.success && response.data !== undefined) {
        return response.data;
    }
    return response;
};

// formatPorts normalises Docker port data from the two shapes the agent
// returns: a comma-separated string (legacy `docker ps`-style output)
// or an array of `{ip, private_port, public_port, type}` objects from
// `docker inspect`. Always returns an array of human-readable strings,
// or `['-']` when there are no ports — both call sites (the container
// list grid and the inspector drawer) want array semantics. Rendering
// the raw inspect array directly triggered React error #31.
export function formatPorts(ports) {
    if (!ports) return ['-'];
    if (Array.isArray(ports)) {
        const formatted = ports
            .map((p) => {
                if (!p || typeof p !== 'object') return null;
                const proto = p.type || p.protocol || 'tcp';
                const priv = p.private_port ?? p.PrivatePort;
                const pub = p.public_port ?? p.PublicPort;
                const ip = p.ip || p.IP;
                if (pub) {
                    return `${ip ? `${ip}:` : ''}${pub}->${priv}/${proto}`;
                }
                return priv ? `${priv}/${proto}` : null;
            })
            .filter(Boolean);
        return formatted.length > 0 ? formatted : ['-'];
    }
    if (typeof ports !== 'string') return ['-'];
    const parts = ports.split(',').map((p) => p.trim()).filter(Boolean);
    return parts.length > 0 ? parts : ['-'];
}

export const normalizeListResponse = (response, key) => {
    const data = unwrapRemoteData(response);
    if (Array.isArray(data)) return data;
    if (Array.isArray(data?.[key])) return data[key];
    return [];
};

export const shortId = (value) => value ? value.substring(0, 12) : '-';

export const getContainerId = (container) => (
    container?.id || container?.ID || container?.Id || ''
);

export const getContainerName = (container) => (
    container?.name || container?.Names || container?.Name || 'unnamed'
);

export const getContainerImage = (container) => (
    container?.image || container?.Image || container?.Config?.Image || '-'
);

export const getContainerStatus = (container) => (
    container?.status || container?.Status || container?.State?.Status || '-'
);

export const getContainerState = (container) => {
    const state = container?.state || container?.State?.Status || container?.State || '';
    return typeof state === 'string' ? state.toLowerCase() : '';
};

export const isContainerRunning = (container) => getContainerState(container) === 'running';

// ServerKit's own infrastructure containers run the panel itself, so we don't
// expose lifecycle controls (start/stop/restart/remove) for them — the backend
// rejects those calls anyway. Trust the backend `protected` flag when present;
// fall back to a name check for shapes (e.g. remote agents) that omit it.
export const SELF_CONTAINER_HINTS = ['serverkit-frontend', 'serverkit_frontend', 'serverkit-backend', 'serverkit_backend'];
export const isProtectedContainer = (container) => {
    if (container?.protected === true) return true;
    const name = getContainerName(container).toLowerCase().replace(/\//g, '');
    return SELF_CONTAINER_HINTS.some(hint => name.includes(hint));
};

export const getContainerStatusLabel = (container) => {
    if (isContainerRunning(container)) return 'Running';
    const state = getContainerState(container);
    if (state === 'exited') return 'Exited';
    if (state === 'created') return 'Created';
    return state || 'Unknown';
};

// Deterministic hue from a container name (demo's per-container accent color).
export const containerHue = (name = '') => {
    let h = 0;
    for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) % 360;
    return h;
};

export const getContainerProjectName = (container, details) => {
    const labels = details?.Config?.Labels || container?.Labels || {};
    return labels['com.docker.compose.project'] || labels['com.docker.compose.service'] || '-';
};
