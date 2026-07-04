// Polymorphic shared resources: tags + shared variable groups.
// Mounts under /shared on the backend (/api/v1/shared).

// ----------------------------------------------------------------- metadata

export async function getSharedResourceTypes() {
    return this.request('/shared/resource-types');
}

// --------------------------------------------------------------------- tags

export async function listResourceTags(resourceType, resourceId) {
    const params = new URLSearchParams({
        resource_type: resourceType,
        resource_id: String(resourceId),
    });
    return this.request(`/shared/tags?${params.toString()}`);
}

export async function listResourcesByTag(tag, resourceType = null) {
    const params = new URLSearchParams({ tag });
    if (resourceType) params.set('resource_type', resourceType);
    return this.request(`/shared/tags?${params.toString()}`);
}

export async function addResourceTag(resourceType, resourceId, tag) {
    return this.request('/shared/tags', {
        method: 'POST',
        body: { resource_type: resourceType, resource_id: String(resourceId), tag },
    });
}

export async function removeResourceTag(resourceType, resourceId, tag) {
    return this.request('/shared/tags', {
        method: 'DELETE',
        body: { resource_type: resourceType, resource_id: String(resourceId), tag },
    });
}

// --------------------------------------------------------- variable groups

export async function listVariableGroups(scopeType = null, scopeId = null) {
    const params = new URLSearchParams();
    if (scopeType) params.set('scope_type', scopeType);
    if (scopeId != null) params.set('scope_id', String(scopeId));
    const qs = params.toString();
    return this.request(`/shared/variable-groups${qs ? `?${qs}` : ''}`);
}

export async function getVariableGroup(groupId) {
    return this.request(`/shared/variable-groups/${groupId}`);
}

export async function createVariableGroup({ scopeType, scopeId, name, description = null }) {
    return this.request('/shared/variable-groups', {
        method: 'POST',
        body: {
            scope_type: scopeType,
            scope_id: String(scopeId),
            name,
            description,
        },
    });
}

export async function updateVariableGroup(groupId, { name, description } = {}) {
    return this.request(`/shared/variable-groups/${groupId}`, {
        method: 'PUT',
        body: { name, description },
    });
}

export async function deleteVariableGroup(groupId) {
    return this.request(`/shared/variable-groups/${groupId}`, { method: 'DELETE' });
}

// ----------------------------------------------- variables within a group

export async function addGroupVariable(groupId, { key, value = '', isSecret = false, targetService = null }) {
    const body = { key, value, is_secret: isSecret };
    // Empty/null target_service means "all services".
    if (targetService) body.target_service = targetService;
    return this.request(`/shared/variable-groups/${groupId}/variables`, {
        method: 'POST',
        body,
    });
}

export async function updateGroupVariable(groupId, variableId, { value, isSecret, targetService } = {}) {
    const body = {};
    if (value !== undefined) body.value = value;
    if (isSecret !== undefined) body.is_secret = isSecret;
    // Passing target_service (even empty string) sets it; empty clears to all services.
    if (targetService !== undefined) body.target_service = targetService;
    return this.request(`/shared/variable-groups/${groupId}/variables/${variableId}`, {
        method: 'PUT',
        body,
    });
}

export async function deleteGroupVariable(groupId, variableId) {
    return this.request(`/shared/variable-groups/${groupId}/variables/${variableId}`, {
        method: 'DELETE',
    });
}

// ----------------------------------------------------------------- attach

export async function attachVariableGroup(groupId, resourceType, resourceId) {
    return this.request(`/shared/variable-groups/${groupId}/attach`, {
        method: 'POST',
        body: { resource_type: resourceType, resource_id: String(resourceId) },
    });
}

export async function detachVariableGroup(groupId, resourceType, resourceId) {
    return this.request(`/shared/variable-groups/${groupId}/detach`, {
        method: 'POST',
        body: { resource_type: resourceType, resource_id: String(resourceId) },
    });
}

// ---------------------------------------------------------------- resolved

export async function getResolvedVariables(resourceType, resourceId) {
    const params = new URLSearchParams({
        resource_type: resourceType,
        resource_id: String(resourceId),
    });
    return this.request(`/shared/resolved?${params.toString()}`);
}

// Hierarchical resolution — each entry carries a `source_scope` provenance
// marker (workspace|project|environment|direct|resource). `context` may carry
// workspace_id/project_id/environment_id to contribute inherited scope layers.
export async function getResolvedVariablesHierarchical(resourceType, resourceId, context = {}) {
    const params = new URLSearchParams({
        resource_type: resourceType,
        resource_id: String(resourceId),
    });
    ['workspace_id', 'project_id', 'environment_id'].forEach((k) => {
        if (context[k] != null && context[k] !== '') params.set(k, String(context[k]));
    });
    return this.request(`/shared/resolved/hierarchical?${params.toString()}`);
}
