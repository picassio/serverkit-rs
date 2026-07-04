import { useEffect, useState } from 'react';
import api from '../../services/api';
import { useAuth } from '../../contexts/AuthContext';
import { Switch } from '@/components/ui/switch';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';
import ProviderLoginCard from './ProviderLoginCard';

const AI_CONFIG_CHANGED_EVENT = 'serverkit:ai-config-changed';

const AISettingsTab = () => {
    const { isAdmin } = useAuth();
    const [settings, setSettings] = useState({
        enabled: false, provider: '', model: '', endpoint: '',
        pii_redaction: true, injection_detection: true, max_cost_usd: '0.5',
        api_key_set: false,
    });
    const [providers, setProviders] = useState([]);
    const [models, setModels] = useState([]);
    const [apiKey, setApiKey] = useState('');     // write-only; '' means "unchanged"
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [testing, setTesting] = useState(false);
    const [message, setMessage] = useState(null);

    useEffect(() => {
        if (!isAdmin) { setLoading(false); return; }
        Promise.all([api.aiGetSettings(), api.aiGetProviders()])
            .then(([s, p]) => {
                setSettings((prev) => ({ ...prev, ...s, max_cost_usd: String(s.max_cost_usd ?? '0.5') }));
                setProviders(p.providers || []);
                if (s.provider) loadModels(s.provider);
            })
            .catch((e) => setMessage({ type: 'error', text: e.message || 'Failed to load AI settings' }))
            .finally(() => setLoading(false));
    }, [isAdmin]);

    const loadModels = (provider) => {
        if (!provider) { setModels([]); return; }
        api.aiGetModels(provider).then((d) => setModels(d.models || [])).catch(() => setModels([]));
    };

    const onProviderChange = (provider) => {
        setSettings((s) => ({ ...s, provider, model: '' }));
        loadModels(provider);
    };

    const buildPayload = () => {
        const payload = {
            enabled: settings.enabled,
            provider: settings.provider,
            model: settings.model,
            endpoint: settings.endpoint,
            pii_redaction: settings.pii_redaction,
            injection_detection: settings.injection_detection,
            max_cost_usd: settings.max_cost_usd,
        };
        if (apiKey.trim()) payload.api_key = apiKey.trim();
        return payload;
    };

    const handleSave = async () => {
        setSaving(true);
        setMessage(null);
        try {
            await api.aiUpdateSettings(buildPayload());
            setApiKey('');
            const fresh = await api.aiGetSettings();
            setSettings((prev) => ({ ...prev, ...fresh, max_cost_usd: String(fresh.max_cost_usd ?? '0.5') }));
            window.dispatchEvent(new Event(AI_CONFIG_CHANGED_EVENT));
            setMessage({ type: 'success', text: 'AI settings saved' });
        } catch (e) {
            setMessage({ type: 'error', text: e.message || 'Failed to save AI settings' });
        } finally {
            setSaving(false);
        }
    };

    const handleTest = async () => {
        setTesting(true);
        setMessage(null);
        try {
            const body = { provider: settings.provider, model: settings.model, endpoint: settings.endpoint };
            if (apiKey.trim()) body.api_key = apiKey.trim();
            const res = await api.aiTestSettings(body);
            setMessage(res.ok
                ? { type: 'success', text: 'Connection OK' }
                : { type: 'error', text: `Connection failed: ${res.error || 'unknown error'}` });
        } catch (e) {
            setMessage({ type: 'error', text: e.message || 'Test failed' });
        } finally {
            setTesting(false);
        }
    };

    if (!isAdmin) {
        return <div className="settings-section"><p>Admin access required.</p></div>;
    }
    if (loading) {
        return <div className="settings-section"><p>Loading…</p></div>;
    }

    const activeProvider = providers.find((p) => p.id === settings.provider);
    const needsKey = activeProvider ? activeProvider.needs_key : true;

    return (
        <div className="settings-section">
            <h2>AI Assistant</h2>
            <p className="section-description">
                Configure the in-panel assistant — powered by Prompture. The API key is stored
                encrypted and never returned by the API.
            </p>

            {message && <div className={`message ${message.type}`}>{message.text}</div>}

            <div className="settings-card">
                <div className="form-group">
                    <div className="settings-row">
                        <div className="settings-label"><Label>Enable AI assistant</Label></div>
                        <Switch
                            checked={settings.enabled}
                            onCheckedChange={(v) => setSettings((s) => ({ ...s, enabled: v }))}
                        />
                    </div>
                </div>

                <div className="form-group">
                    <label htmlFor="ai-provider">Provider</label>
                    <select
                        id="ai-provider"
                        value={settings.provider}
                        onChange={(e) => onProviderChange(e.target.value)}
                    >
                        <option value="">Select a provider…</option>
                        {providers.map((p) => (
                            <option key={p.id} value={p.id}>{p.label}</option>
                        ))}
                    </select>
                </div>

                <div className="form-group">
                    <label htmlFor="ai-model">Model</label>
                    <select
                        id="ai-model"
                        value={settings.model}
                        disabled={!settings.provider}
                        onChange={(e) => setSettings((s) => ({ ...s, model: e.target.value }))}
                    >
                        <option value="">
                            {!settings.provider
                                ? 'Select a provider first…'
                                : models.length
                                    ? 'Select a model…'
                                    : 'No models available for this provider'}
                        </option>
                        {settings.model && !models.includes(settings.model) && (
                            <option value={settings.model}>{settings.model} (current)</option>
                        )}
                        {models.map((m) => <option key={m} value={m}>{m}</option>)}
                    </select>
                </div>

                {needsKey && (
                    <div className="form-group">
                        <label htmlFor="ai-key">API key</label>
                        <input
                            id="ai-key"
                            type="password"
                            autoComplete="off"
                            placeholder={settings.api_key_set ? 'Configured ✓ (leave blank to keep)' : 'Paste your API key'}
                            value={apiKey}
                            onChange={(e) => setApiKey(e.target.value)}
                        />
                    </div>
                )}

                {activeProvider && activeProvider.supports_endpoint && (
                    <div className="form-group">
                        <label htmlFor="ai-endpoint">Endpoint (optional)</label>
                        <input
                            id="ai-endpoint"
                            type="text"
                            placeholder="http://localhost:11434"
                            value={settings.endpoint}
                            onChange={(e) => setSettings((s) => ({ ...s, endpoint: e.target.value }))}
                        />
                    </div>
                )}

                <div className="form-group">
                    <label htmlFor="ai-max-cost">Per-conversation cost ceiling (USD)</label>
                    <input
                        id="ai-max-cost"
                        type="number"
                        step="0.1"
                        min="0"
                        value={settings.max_cost_usd}
                        onChange={(e) => setSettings((s) => ({ ...s, max_cost_usd: e.target.value }))}
                    />
                </div>

                <div className="form-group">
                    <div className="settings-row">
                        <div className="settings-label"><Label>Redact PII from messages &amp; tool output</Label></div>
                        <Switch
                            checked={settings.pii_redaction}
                            onCheckedChange={(v) => setSettings((s) => ({ ...s, pii_redaction: v }))}
                        />
                    </div>
                </div>
                <div className="form-group">
                    <div className="settings-row">
                        <div className="settings-label"><Label>Block prompt-injection attempts</Label></div>
                        <Switch
                            checked={settings.injection_detection}
                            onCheckedChange={(v) => setSettings((s) => ({ ...s, injection_detection: v }))}
                        />
                    </div>
                </div>

                <div className="settings-actions" style={{ display: 'flex', gap: '8px', marginTop: '12px' }}>
                    <Button variant="outline" onClick={handleTest} disabled={testing || !settings.provider || !settings.model}>
                        {testing ? 'Testing…' : 'Test connection'}
                    </Button>
                    <Button variant="primary" onClick={handleSave} disabled={saving}>
                        {saving ? 'Saving…' : 'Save'}
                    </Button>
                </div>
            </div>

            <ProviderLoginCard />
        </div>
    );
};

export default AISettingsTab;
