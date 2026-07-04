// Base HTTP client - constructor, token management, core request methods
const AUTH_EXPIRED_EVENT = 'serverkit:auth-expired';

const normalizeApiBaseUrl = (url) => {
    if (!url) return '/api/v1';
    const trimmed = url.replace(/\/+$/, '');
    return trimmed.endsWith('/api/v1') ? trimmed : `${trimmed}/api/v1`;
};

// In dev, keep browser requests same-origin so Vite can proxy /api to the
// configured backend target. Calling VITE_API_URL directly bypasses that
// proxy and exposes CORS/preflight redirects in the browser.
const API_BASE_URL = import.meta.env.DEV
    ? '/api/v1'
    : normalizeApiBaseUrl(import.meta.env.VITE_API_URL);

class ApiClient {
    constructor() {
        this.baseUrl = API_BASE_URL;
    }

    getToken() {
        return localStorage.getItem('access_token');
    }

    setTokens(accessToken, refreshToken) {
        localStorage.setItem('access_token', accessToken);
        localStorage.setItem('refresh_token', refreshToken);
    }

    clearTokens() {
        localStorage.removeItem('access_token');
        localStorage.removeItem('refresh_token');
        localStorage.removeItem('active_workspace_id');  // drop workspace context on logout
        localStorage.removeItem('active_workspace');
        localStorage.removeItem('workspace_accent');
    }

    async request(endpoint, options = {}) {
        const url = `${this.baseUrl}${endpoint}`;
        const token = this.getToken();

        // FormData uploads need browser-built Content-Type (with boundary)
        // and must NOT be JSON-stringified. Detect and bypass both.
        const isFormData = options.body instanceof FormData;

        // Active workspace context (#33). Sent ambiently so the backend can scope
        // resources; endpoints that don't honor it ignore it. A stale value is safe
        // — the backend resolves it leniently (falls back to no scope).
        const activeWorkspace = localStorage.getItem('active_workspace_id');

        // Spread `...options` FIRST, then set merged headers LAST — otherwise a
        // call passing custom `headers` (e.g. X-DB-Password) would clobber the
        // whole merged set, dropping Content-Type/Authorization and triggering
        // 415 (no application/json) on JSON POSTs.
        const config = {
            ...options,
            headers: {
                ...(isFormData ? {} : { 'Content-Type': 'application/json' }),
                ...(token && { Authorization: `Bearer ${token}` }),
                ...(activeWorkspace && activeWorkspace !== 'all' && { 'X-Workspace-Id': activeWorkspace }),
                ...options.headers,
            },
        };

        if (
            !isFormData &&
            options.body &&
            typeof options.body === 'object' &&
            !(options.body instanceof Blob)
        ) {
            config.body = JSON.stringify(options.body);
        }

        const response = await fetch(url, config);

        if (response.status === 401) {
            // flask-jwt-extended returns 401 with `{"msg": "..."}` when
            // the token is the problem; domain endpoints (e.g. wrong
            // pair-code passphrase) return `{"error": "..."}`. Only the
            // former should trigger a token refresh — refreshing on a
            // domain 401 wastes a backend round-trip and burns through
            // rate limits twice as fast.
            const probe = await response.clone().json().catch(() => ({}));
            const isJwtIssue = probe && probe.msg && !probe.error;

            if (isJwtIssue) {
                const refreshed = await this.refreshToken();
                if (refreshed) {
                    config.headers.Authorization = `Bearer ${this.getToken()}`;
                    const retryResponse = await fetch(url, config);
                    return this.handleResponse(retryResponse);
                }
                this.clearTokens();
                window.dispatchEvent(new Event(AUTH_EXPIRED_EVENT));
                const err = new Error('Session expired');
                err.status = 401;
                throw err;
            }
            // Domain 401 — fall through so handleResponse throws the
            // server's error message verbatim, with status attached.
        }

        return this.handleResponse(response);
    }

    async handleResponse(response) {
        const data = await response.json().catch(() => ({}));
        if (!response.ok) {
            const err = new Error(data.error || data.msg || 'Request failed');
            err.status = response.status;
            err.data = data;
            throw err;
        }
        return data;
    }

    async refreshToken() {
        const refreshToken = localStorage.getItem('refresh_token');
        if (!refreshToken) return false;

        try {
            const response = await fetch(`${this.baseUrl}/auth/refresh`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    Authorization: `Bearer ${refreshToken}`,
                },
            });

            if (response.ok) {
                const data = await response.json();
                localStorage.setItem('access_token', data.access_token);
                return true;
            }
            return false;
        } catch {
            return false;
        }
    }
}

export default ApiClient;
