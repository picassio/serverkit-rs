// Notification Bus API methods. Mixed into ApiService (see ./index.js) so each
// function runs with `this` bound to the client and calls `this.request(...)`.

// --- In-app notification center (the bell + history) ---

export async function getInbox(params = {}) {
    const query = new URLSearchParams();
    if (params.limit) query.append('limit', params.limit);
    if (params.offset) query.append('offset', params.offset);
    if (params.unread) query.append('unread', '1');
    const suffix = query.toString() ? `?${query}` : '';
    return this.request(`/notifications/inbox${suffix}`);
}

export async function getNotificationUnreadCount() {
    return this.request('/notifications/inbox/unread-count');
}

export async function markNotificationRead(deliveryId) {
    return this.request(`/notifications/inbox/${deliveryId}/read`, { method: 'POST' });
}

export async function markAllNotificationsRead() {
    return this.request('/notifications/inbox/read-all', { method: 'POST' });
}

// --- Delivery log / ops (admin) ---

export async function getDeliveryLog(params = {}) {
    const query = new URLSearchParams();
    if (params.status) query.append('status', params.status);
    if (params.channel) query.append('channel', params.channel);
    if (params.limit) query.append('limit', params.limit);
    if (params.offset) query.append('offset', params.offset);
    const suffix = query.toString() ? `?${query}` : '';
    return this.request(`/notifications/admin/deliveries${suffix}`);
}

export async function retryDelivery(deliveryId) {
    return this.request(`/notifications/admin/deliveries/${deliveryId}/retry`, { method: 'POST' });
}

// --- Email provider integrations (admin) ---

export async function getEmailProviders() {
    return this.request('/notifications/admin/email-providers');
}

export async function addEmailProvider(data) {
    return this.request('/notifications/admin/email-providers', {
        method: 'POST',
        body: JSON.stringify(data),
    });
}

export async function testEmailProvider(providerId) {
    return this.request(`/notifications/admin/email-providers/${providerId}/test`, { method: 'POST' });
}

export async function setDefaultEmailProvider(providerId) {
    return this.request(`/notifications/admin/email-providers/${providerId}/default`, { method: 'POST' });
}

export async function deleteEmailProvider(providerId) {
    return this.request(`/notifications/admin/email-providers/${providerId}`, { method: 'DELETE' });
}
