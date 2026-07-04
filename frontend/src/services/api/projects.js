// Projects & Environments (Workspace -> Project -> Environment -> Applications)
//
// Project endpoints are workspace-scoped; the active workspace is derived from
// the X-Workspace-Id header the client already sends, so these methods don't
// need to pass it explicitly.

// --- Projects ---

export async function getProjects() {
    return this.request('/projects');
}

export async function getProject(id) {
    return this.request(`/projects/${id}`);
}

export async function createProject(data) {
    return this.request('/projects', {
        method: 'POST',
        body: data,
    });
}

export async function updateProject(id, data) {
    return this.request(`/projects/${id}`, {
        method: 'PUT',
        body: data,
    });
}

export async function deleteProject(id) {
    return this.request(`/projects/${id}`, {
        method: 'DELETE',
    });
}

// --- Environments ---

export async function createEnvironment(projectId, data) {
    return this.request('/environments', {
        method: 'POST',
        body: { project_id: projectId, ...data },
    });
}

export async function updateEnvironment(environmentId, data) {
    return this.request(`/environments/${environmentId}`, {
        method: 'PUT',
        body: data,
    });
}

export async function deleteEnvironment(environmentId) {
    return this.request(`/environments/${environmentId}`, {
        method: 'DELETE',
    });
}

export async function reorderEnvironments(projectId, orderedIds) {
    return this.request('/environments/reorder', {
        method: 'POST',
        body: { project_id: projectId, ordered_ids: orderedIds },
    });
}

// --- Apps <-> Project assignment ---

// Bulk-assign applications to a project (and optionally an environment), or
// unassign them by passing projectId = null. The backend validates the
// project/environment against each app's workspace and silently drops invalid
// pairings, so a mixed selection is safe. Returns { apps, skipped }.
export async function moveAppsToProject(appIds, projectId, environmentId) {
    return this.request('/apps/move-to-project', {
        method: 'POST',
        body: {
            app_ids: appIds,
            project_id: projectId ?? null,
            environment_id: environmentId ?? null,
        },
    });
}
