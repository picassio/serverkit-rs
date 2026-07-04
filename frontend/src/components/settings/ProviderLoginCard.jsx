import { useEffect, useState } from 'react';
import api from '../../services/api';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';

// Login flow for OAuth/subscription providers (Claude Pro/Max, Copilot, Codex).
// Uses the sidecar's AuthStorage.login via the Rust /ai/auth/* proxy. Because a
// remote browser can't hit the sidecar's localhost callback, we surface the
// authorize URL and let the operator paste back the final redirect URL/code.
const ProviderLoginCard = () => {
    const [providers, setProviders] = useState([]);
    const [available, setAvailable] = useState(true);
    const [loading, setLoading] = useState(true);
    const [flow, setFlow] = useState(null); // { provider, login_id, url, instructions }
    const [pasted, setPasted] = useState('');
    const [busy, setBusy] = useState(false);
    const [message, setMessage] = useState(null);

    const refresh = () => {
        setLoading(true);
        api.aiAuthStatus()
            .then((d) => { setProviders(d.providers || []); setAvailable(true); })
            .catch((e) => {
                if (e.status === 503) setAvailable(false);
                else setMessage({ type: 'error', text: e.message || 'Failed to load provider status' });
            })
            .finally(() => setLoading(false));
    };
    useEffect(refresh, []);

    const startLogin = (provider) => {
        setBusy(true); setMessage(null); setPasted('');
        api.aiAuthLoginStart(provider)
            .then((d) => setFlow({ provider, ...d }))
            .catch((e) => setMessage({ type: 'error', text: e.message || 'Could not start login' }))
            .finally(() => setBusy(false));
    };

    const completeLogin = () => {
        if (!flow) return;
        setBusy(true); setMessage(null);
        api.aiAuthLoginComplete(flow.login_id, pasted.trim())
            .then(() => {
                setMessage({ type: 'success', text: `Logged in to ${flow.provider} ✓` });
                setFlow(null); setPasted(''); refresh();
            })
            .catch((e) => setMessage({ type: 'error', text: e.message || 'Login failed' }))
            .finally(() => setBusy(false));
    };

    const logout = (provider) => {
        setBusy(true);
        api.aiAuthLogout(provider).then(refresh).finally(() => setBusy(false));
    };

    if (!available) {
        return (
            <div className="settings-card" style={{ marginTop: 16 }}>
                <h3>Provider Login</h3>
                <p className="section-description">
                    The AI sidecar is not running, so subscription logins (Claude Pro/Max) are
                    unavailable. Start the sidecar to enable browser-based login.
                </p>
            </div>
        );
    }

    return (
        <div className="settings-card" style={{ marginTop: 16 }}>
            <h3>Provider Login</h3>
            <p className="section-description">
                Sign in with a subscription (Claude Pro/Max, ChatGPT, Copilot) instead of an API key.
                Credentials are stored on the server; nothing is entered here except the one-time code.
            </p>

            {message && <div className={`message ${message.type}`}>{message.text}</div>}

            {loading ? (
                <p>Loading…</p>
            ) : (
                <div className="provider-login-list" style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                    {providers.map((p) => (
                        <div key={p.id} className="settings-row" style={{ alignItems: 'center' }}>
                            <div className="settings-label">
                                <Label>{p.name}</Label>{' '}
                                {p.configured
                                    ? <span style={{ color: 'var(--success, #16a34a)' }}>● Connected</span>
                                    : <span style={{ opacity: 0.6 }}>○ Not connected</span>}
                            </div>
                            <div style={{ display: 'flex', gap: 8 }}>
                                <Button variant="outline" disabled={busy} onClick={() => startLogin(p.id)}>
                                    {p.configured ? 'Re-login' : 'Login'}
                                </Button>
                                {p.configured && (
                                    <Button variant="ghost" disabled={busy} onClick={() => logout(p.id)}>
                                        Logout
                                    </Button>
                                )}
                            </div>
                        </div>
                    ))}
                </div>
            )}

            {flow && (
                <div className="login-flow" style={{ marginTop: 12, padding: 12, border: '1px solid var(--border, #333)', borderRadius: 8 }}>
                    <p><strong>1.</strong> Open this URL and authorize:</p>
                    <p style={{ wordBreak: 'break-all' }}>
                        <a href={flow.url} target="_blank" rel="noreferrer">{flow.url}</a>
                    </p>
                    <p><strong>2.</strong> {flow.instructions || 'Paste the final redirect URL (or the code) below.'}</p>
                    <textarea
                        rows={3}
                        style={{ width: '100%' }}
                        placeholder="https://localhost/callback?code=…  (or just the code)"
                        value={pasted}
                        onChange={(e) => setPasted(e.target.value)}
                    />
                    <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
                        <Button variant="primary" disabled={busy || !pasted.trim()} onClick={completeLogin}>
                            {busy ? 'Completing…' : 'Complete login'}
                        </Button>
                        <Button variant="ghost" disabled={busy} onClick={() => { setFlow(null); setPasted(''); }}>
                            Cancel
                        </Button>
                    </div>
                </div>
            )}
        </div>
    );
};

export default ProviderLoginCard;
