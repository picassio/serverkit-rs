// Telemetry / system event stream API methods

export async function getTelemetryEvents(params = {}) {
    const query = new URLSearchParams();
    if (params.page) query.append('page', params.page);
    if (params.per_page) query.append('per_page', params.per_page);
    if (params.source) query.append('source', params.source);
    if (params.event_type) query.append('event_type', params.event_type);
    if (params.severity) query.append('severity', params.severity);
    if (params.resource_type) query.append('resource_type', params.resource_type);
    if (params.resource_id) query.append('resource_id', params.resource_id);
    if (params.correlation_id) query.append('correlation_id', params.correlation_id);
    if (params.start_date) query.append('start_date', params.start_date);
    if (params.end_date) query.append('end_date', params.end_date);
    if (params.q) query.append('q', params.q);
    const suffix = query.toString() ? `?${query}` : '';
    return this.request(`/telemetry/events${suffix}`);
}

export async function getTelemetryEvent(eventId) {
    return this.request(`/telemetry/events/${eventId}`);
}

export async function getTelemetryEventsByCorrelation(correlationId, params = {}) {
    const query = new URLSearchParams();
    if (params.limit) query.append('limit', params.limit);
    const suffix = query.toString() ? `?${query}` : '';
    return this.request(`/telemetry/events/by-correlation/${correlationId}${suffix}`);
}

export async function getTelemetryStats(params = {}) {
    const query = new URLSearchParams();
    if (params.hours) query.append('hours', params.hours);
    if (params.source) query.append('source', params.source);
    const suffix = query.toString() ? `?${query}` : '';
    return this.request(`/telemetry/stats${suffix}`);
}

export async function getTelemetrySources() {
    return this.request('/telemetry/sources');
}

export async function getTelemetryEventTypes(params = {}) {
    const query = new URLSearchParams();
    if (params.source) query.append('source', params.source);
    const suffix = query.toString() ? `?${query}` : '';
    return this.request(`/telemetry/event-types${suffix}`);
}

export async function cleanupTelemetryEvents(days = 90) {
    return this.request(`/telemetry/events?days=${days}`, { method: 'DELETE' });
}

export async function emitTestTelemetryEvent(data = {}) {
    return this.request('/telemetry/events/test', {
        method: 'POST',
        body: JSON.stringify(data),
    });
}
