// Docker containers, images, volumes, networks, compose operations

// Docker status
export async function getDockerStatus() {
    return this.request('/docker/status');
}

export async function getDockerInfo() {
    return this.request('/docker/info');
}

export async function getDockerDiskUsage() {
    return this.request('/docker/disk-usage');
}

// Containers
export async function getContainers(all = true) {
    return this.request(`/docker/containers?all=${all}`);
}

export async function getContainer(containerId) {
    return this.request(`/docker/containers/${containerId}`);
}

export async function createContainer(data) {
    return this.request('/docker/containers', {
        method: 'POST',
        body: data
    });
}

export async function runContainer(data) {
    return this.request('/docker/containers/run', {
        method: 'POST',
        body: data
    });
}

export async function startContainer(containerId) {
    return this.request(`/docker/containers/${containerId}/start`, { method: 'POST' });
}

export async function stopContainer(containerId, timeout = 10) {
    return this.request(`/docker/containers/${containerId}/stop`, {
        method: 'POST',
        body: { timeout }
    });
}

export async function restartContainer(containerId, timeout = 10) {
    return this.request(`/docker/containers/${containerId}/restart`, {
        method: 'POST',
        body: { timeout }
    });
}

export async function removeContainer(containerId, force = false, volumes = false) {
    return this.request(`/docker/containers/${containerId}`, {
        method: 'DELETE',
        body: { force, volumes }
    });
}

export async function getContainerLogs(containerId, tail = 100, since = null) {
    const params = new URLSearchParams({ tail });
    if (since) params.append('since', since);
    return this.request(`/docker/containers/${containerId}/logs?${params}`);
}

export async function getContainerStats(containerId) {
    return this.request(`/docker/containers/${containerId}/stats`);
}

export async function getContainersStats(containerIds) {
    return this.request('/docker/containers/stats', {
        method: 'POST',
        body: { ids: containerIds }
    });
}

export async function execContainer(containerId, command) {
    return this.request(`/docker/containers/${containerId}/exec`, {
        method: 'POST',
        body: { command }
    });
}

// Images
export async function getImages() {
    return this.request('/docker/images');
}

export async function pullImage(image, tag = 'latest') {
    return this.request('/docker/images/pull', {
        method: 'POST',
        body: { image, tag }
    });
}

export async function removeImage(imageId, force = false) {
    return this.request(`/docker/images/${imageId}`, {
        method: 'DELETE',
        body: { force }
    });
}

export async function buildImage(path, tag, dockerfile = 'Dockerfile', noCache = false) {
    return this.request('/docker/images/build', {
        method: 'POST',
        body: { path, tag, dockerfile, no_cache: noCache }
    });
}

// Networks
export async function getNetworks() {
    return this.request('/docker/networks');
}

export async function createNetwork(name, driver = 'bridge') {
    return this.request('/docker/networks', {
        method: 'POST',
        body: { name, driver }
    });
}

export async function removeNetwork(networkId) {
    return this.request(`/docker/networks/${networkId}`, { method: 'DELETE' });
}

// Volumes
export async function getVolumes() {
    return this.request('/docker/volumes');
}

export async function createVolume(name, driver = 'local') {
    return this.request('/docker/volumes', {
        method: 'POST',
        body: { name, driver }
    });
}

export async function removeVolume(volumeName, force = false) {
    return this.request(`/docker/volumes/${volumeName}`, {
        method: 'DELETE',
        body: { force }
    });
}

// Docker Compose
export async function composeList() {
    return this.request('/docker/compose/list');
}

export async function composeUp(path, detach = true, build = false) {
    return this.request('/docker/compose/up', {
        method: 'POST',
        body: { path, detach, build }
    });
}

export async function composeDown(path, volumes = false, removeOrphans = true) {
    return this.request('/docker/compose/down', {
        method: 'POST',
        body: { path, volumes, remove_orphans: removeOrphans }
    });
}

export async function composePs(path) {
    return this.request('/docker/compose/ps', {
        method: 'POST',
        body: { path }
    });
}

export async function composeLogs(path, service = null, tail = 100) {
    return this.request('/docker/compose/logs', {
        method: 'POST',
        body: { path, service, tail }
    });
}

export async function composeRestart(path, service = null) {
    return this.request('/docker/compose/restart', {
        method: 'POST',
        body: { path, service }
    });
}

export async function composePull(path, service = null) {
    return this.request('/docker/compose/pull', {
        method: 'POST',
        body: { path, service }
    });
}

// Docker App
export async function createDockerApp(data) {
    return this.request('/docker/apps', {
        method: 'POST',
        body: data
    });
}

export async function pruneDocker(all = false, volumes = false) {
    return this.request('/docker/prune', {
        method: 'POST',
        body: { all, volumes }
    });
}

export async function dockerCleanup(includeVolumes = false) {
    return this.request('/docker/cleanup', {
        method: 'POST',
        body: { volumes: includeVolumes }
    });
}

export async function cleanupAllApps() {
    return this.request('/docker/cleanup/apps', {
        method: 'POST'
    });
}
