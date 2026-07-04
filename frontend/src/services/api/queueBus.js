// Queue Bus API methods

export async function getQueueGroups(params = {}) {
    const query = new URLSearchParams();
    if (params.owner_type) query.append('owner_type', params.owner_type);
    if (params.owner_id) query.append('owner_id', params.owner_id);
    if (params.limit) query.append('limit', params.limit);
    if (params.offset) query.append('offset', params.offset);
    const suffix = query.toString() ? `?${query}` : '';
    return this.request(`/queue/groups${suffix}`);
}

export async function createQueueGroup(data) {
    return this.request('/queue/groups', { method: 'POST', body: JSON.stringify(data) });
}

export async function getQueueGroup(groupSlug) {
    return this.request(`/queue/groups/${groupSlug}`);
}

export async function updateQueueGroup(groupSlug, data) {
    return this.request(`/queue/groups/${groupSlug}`, { method: 'PATCH', body: JSON.stringify(data) });
}

export async function deleteQueueGroup(groupSlug) {
    return this.request(`/queue/groups/${groupSlug}`, { method: 'DELETE' });
}

export async function getQueues(groupSlug, params = {}) {
    const query = new URLSearchParams();
    if (params.limit) query.append('limit', params.limit);
    if (params.offset) query.append('offset', params.offset);
    const suffix = query.toString() ? `?${query}` : '';
    return this.request(`/queue/groups/${groupSlug}/queues${suffix}`);
}

export async function createQueue(groupSlug, data) {
    return this.request(`/queue/groups/${groupSlug}/queues`, { method: 'POST', body: JSON.stringify(data) });
}

export async function getQueue(groupSlug, queueSlug) {
    return this.request(`/queue/groups/${groupSlug}/queues/${queueSlug}`);
}

export async function updateQueue(groupSlug, queueSlug, data) {
    return this.request(`/queue/groups/${groupSlug}/queues/${queueSlug}`, { method: 'PATCH', body: JSON.stringify(data) });
}

export async function deleteQueue(groupSlug, queueSlug) {
    return this.request(`/queue/groups/${groupSlug}/queues/${queueSlug}`, { method: 'DELETE' });
}

export async function getMessages(groupSlug, queueSlug, params = {}) {
    const query = new URLSearchParams();
    if (params.status) query.append('status', params.status);
    if (params.limit) query.append('limit', params.limit);
    if (params.offset) query.append('offset', params.offset);
    const suffix = query.toString() ? `?${query}` : '';
    return this.request(`/queue/groups/${groupSlug}/queues/${queueSlug}/messages${suffix}`);
}

export async function sendMessage(groupSlug, queueSlug, payload, options = {}) {
    return this.request(`/queue/groups/${groupSlug}/queues/${queueSlug}/messages`, {
        method: 'POST',
        body: JSON.stringify({
            payload,
            priority: options.priority ?? 0,
            delay_ms: options.delay_ms ?? 0,
            max_attempts: options.max_attempts,
        }),
    });
}

export async function receiveMessages(groupSlug, queueSlug, options = {}) {
    return this.request(`/queue/groups/${groupSlug}/queues/${queueSlug}/messages/receive`, {
        method: 'POST',
        body: JSON.stringify({
            visibility_timeout_ms: options.visibility_timeout_ms ?? 30000,
            max_messages: options.max_messages ?? 1,
        }),
    });
}

export async function getMessage(groupSlug, queueSlug, messageId) {
    return this.request(`/queue/groups/${groupSlug}/queues/${queueSlug}/messages/${messageId}`);
}

export async function completeMessage(groupSlug, queueSlug, messageId) {
    return this.request(`/queue/groups/${groupSlug}/queues/${queueSlug}/messages/${messageId}/complete`, { method: 'POST' });
}

export async function failMessage(groupSlug, queueSlug, messageId, options = {}) {
    return this.request(`/queue/groups/${groupSlug}/queues/${queueSlug}/messages/${messageId}/fail`, {
        method: 'POST',
        body: JSON.stringify({
            error_message: options.error_message,
            requeue: options.requeue ?? false,
        }),
    });
}

export async function requeueMessage(groupSlug, queueSlug, messageId) {
    return this.request(`/queue/groups/${groupSlug}/queues/${queueSlug}/messages/${messageId}/requeue`, { method: 'POST' });
}

export async function deleteMessage(groupSlug, queueSlug, messageId) {
    return this.request(`/queue/groups/${groupSlug}/queues/${queueSlug}/messages/${messageId}`, { method: 'DELETE' });
}

export async function getQueueStats(groupSlug, queueSlug) {
    return this.request(`/queue/groups/${groupSlug}/queues/${queueSlug}/stats`);
}

export async function getGroupStats(groupSlug) {
    return this.request(`/queue/groups/${groupSlug}/stats`);
}

export async function getGlobalQueueStats() {
    return this.request('/queue/stats');
}
