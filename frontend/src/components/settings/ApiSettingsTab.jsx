import { useState, useEffect, useCallback } from 'react';
import { useAuth } from '../../contexts/AuthContext';
import api from '../../services/api';
import ApiKeyModal from './ApiKeyModal';
import WebhookSubscriptionModal from './WebhookSubscriptionModal';
import {
    Key, Plus, Trash2, RotateCcw, Activity, AlertCircle,
    Check, Send, ChevronDown, ChevronUp, Zap, BarChart3, RefreshCw
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { Pill } from '@/components/ds/Pill';

const ApiSettingsTab = () => {
    const { isAdmin } = useAuth();

    return (
        <div className="api-settings">
            <ApiKeysSection />
            {isAdmin && <RateLimitsSection />}
            <WebhookSection />
            {isAdmin && <AnalyticsSection />}
        </div>
    );
};

// ─── API Keys Section ──────────────────────────────────
const ApiKeysSection = () => {
    const [keys, setKeys] = useState([]);
    const [loading, setLoading] = useState(true);
    const [showModal, setShowModal] = useState(false);
    const [createdKey, setCreatedKey] = useState(null);

    const loadKeys = () => {
        setLoading(true);
        api.getApiKeys().then(data => {
            setKeys(data.api_keys || []);
        }).catch(() => {}).finally(() => setLoading(false));
    };

    useEffect(() => { loadKeys(); }, []);

    const handleCreate = async (data) => {
        const result = await api.createApiKey(data);
        setCreatedKey(result.raw_key);
        loadKeys();
    };

    const handleRevoke = async (keyId) => {
        if (!confirm('Revoke this API key? This cannot be undone.')) return;
        await api.revokeApiKey(keyId);
        loadKeys();
    };

    const handleRotate = async (keyId) => {
        if (!confirm('Rotate this key? The old key will stop working immediately.')) return;
        const result = await api.rotateApiKey(keyId);
        setCreatedKey(result.raw_key);
        setShowModal(true);
        loadKeys();
    };

    const closeModal = () => {
        setShowModal(false);
        setCreatedKey(null);
    };

    return (
        <div className="settings-card">
            <div className="settings-card__header">
                <div className="settings-card__header-left">
                    <Key size={20} />
                    <div>
                        <h3>API Keys</h3>
                        <p>Manage programmatic access to the ServerKit API</p>
                    </div>
                </div>
                <Button variant="default" size="sm" onClick={() => setShowModal(true)}>
                    <Plus size={14} /> Create Key
                </Button>
            </div>

            {loading ? (
                <div className="settings-card__loading">Loading...</div>
            ) : keys.length === 0 ? (
                <div className="settings-card__empty">
                    <Key size={24} />
                    <p>No API keys yet. Create one to get started.</p>
                </div>
            ) : (
                <div className="api-settings__table-wrap">
                    <table className="api-settings__table">
                        <thead>
                            <tr>
                                <th>Name</th>
                                <th>Key</th>
                                <th>Scopes</th>
                                <th>Tier</th>
                                <th>Last Used</th>
                                <th>Status</th>
                                <th>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {keys.map(key => (
                                <tr key={key.id}>
                                    <td className="api-settings__key-name">{key.name}</td>
                                    <td><code className="api-settings__key-prefix">{key.key_prefix}...</code></td>
                                    <td>
                                        <div className="api-settings__scopes">
                                            {(!key.scopes || key.scopes.length === 0) ? (
                                                <span className="api-settings__muted">None</span>
                                            ) : key.scopes.includes('*') ? (
                                                <Pill kind="violet">Full access</Pill>
                                            ) : (
                                                <>
                                                    {key.scopes.slice(0, 3).map(s => (
                                                        <Pill key={s} kind="cyan" dot={false}>{s}</Pill>
                                                    ))}
                                                    {key.scopes.length > 3 && (
                                                        <Pill kind="gray" dot={false}>+{key.scopes.length - 3}</Pill>
                                                    )}
                                                </>
                                            )}
                                        </div>
                                    </td>
                                    <td>
                                        <Badge variant="outline">{key.tier}</Badge>
                                    </td>
                                    <td className="api-settings__muted">
                                        {key.last_used_at
                                            ? new Date(key.last_used_at).toLocaleDateString()
                                            : 'Never'}
                                    </td>
                                    <td>
                                        {key.is_active && !key.revoked_at ? (
                                            <Badge variant="success">Active</Badge>
                                        ) : (
                                            <Badge variant="destructive">Revoked</Badge>
                                        )}
                                    </td>
                                    <td className="api-settings__actions">
                                        {key.is_active && !key.revoked_at && (
                                            <>
                                                <Button
                                                    variant="ghost"
                                                    size="sm"
                                                    onClick={() => handleRotate(key.id)}
                                                    title="Rotate"
                                                >
                                                    <RotateCcw size={14} />
                                                </Button>
                                                <Button
                                                    variant="ghost"
                                                    size="sm"
                                                    onClick={() => handleRevoke(key.id)}
                                                    title="Revoke"
                                                    className="text-destructive hover:text-destructive"
                                                >
                                                    <Trash2 size={14} />
                                                </Button>
                                            </>
                                        )}
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            )}

            {(showModal || createdKey) && (
                <ApiKeyModal
                    onClose={closeModal}
                    onSubmit={handleCreate}
                    createdKey={createdKey}
                />
            )}
        </div>
    );
};

// ─── Rate Limits Section ───────────────────────────────
const RateLimitsSection = () => {
    const [limits, setLimits] = useState({
        rate_limit_standard: '100 per minute',
        rate_limit_elevated: '500 per minute',
        rate_limit_unlimited: '5000 per minute',
        rate_limit_unauthenticated: '30 per minute',
    });
    const [saving, setSaving] = useState(false);
    const [message, setMessage] = useState(null);

    useEffect(() => {
        api.getSystemSettings().then(data => {
            setLimits(prev => ({
                rate_limit_standard: data.rate_limit_standard || prev.rate_limit_standard,
                rate_limit_elevated: data.rate_limit_elevated || prev.rate_limit_elevated,
                rate_limit_unlimited: data.rate_limit_unlimited || prev.rate_limit_unlimited,
                rate_limit_unauthenticated: data.rate_limit_unauthenticated || prev.rate_limit_unauthenticated,
            }));
        }).catch(() => {});
    }, []);

    const handleSave = async () => {
        setSaving(true);
        setMessage(null);
        try {
            for (const [key, value] of Object.entries(limits)) {
                await api.updateSystemSetting(key, value);
            }
            setMessage({ type: 'success', text: 'Rate limits updated' });
        } catch {
            setMessage({ type: 'error', text: 'Failed to update rate limits' });
        } finally {
            setSaving(false);
        }
    };

    const labels = {
        rate_limit_standard: 'Standard Tier',
        rate_limit_elevated: 'Elevated Tier',
        rate_limit_unlimited: 'Unlimited Tier',
        rate_limit_unauthenticated: 'Unauthenticated',
    };

    return (
        <div className="settings-card">
            <div className="settings-card__header">
                <div className="settings-card__header-left">
                    <Activity size={20} />
                    <div>
                        <h3>Rate Limits</h3>
                        <p>Configure request rate limits by tier</p>
                    </div>
                </div>
            </div>

            {message && (
                <div className={`alert alert--${message.type}`}>
                    {message.type === 'success' ? <Check size={16} /> : <AlertCircle size={16} />}
                    {message.text}
                </div>
            )}

            <div className="api-settings__rate-limits">
                {Object.entries(limits).map(([key, value]) => (
                    <div key={key} className="form-group form-group--inline">
                        <Label>{labels[key]}</Label>
                        <Input
                            type="text"
                            value={value}
                            onChange={e => setLimits(prev => ({ ...prev, [key]: e.target.value }))}
                            placeholder="e.g. 100 per minute"
                        />
                    </div>
                ))}
            </div>

            <div className="settings-card__footer">
                <Button variant="default" onClick={handleSave} disabled={saving}>
                    {saving ? 'Saving...' : 'Save Rate Limits'}
                </Button>
            </div>
        </div>
    );
};

// ─── Webhook Subscriptions Section ─────────────────────
const WebhookSection = () => {
    const [subscriptions, setSubscriptions] = useState([]);
    const [loading, setLoading] = useState(true);
    const [showModal, setShowModal] = useState(false);
    const [editingSub, setEditingSub] = useState(null);
    const [expandedId, setExpandedId] = useState(null);
    const [deliveries, setDeliveries] = useState({});

    const loadSubscriptions = () => {
        setLoading(true);
        api.getEventSubscriptions().then(data => {
            setSubscriptions(data.subscriptions || []);
        }).catch(() => {}).finally(() => setLoading(false));
    };

    useEffect(() => { loadSubscriptions(); }, []);

    const handleCreate = async (data) => {
        await api.createEventSubscription(data);
        setShowModal(false);
        loadSubscriptions();
    };

    const handleUpdate = async (data) => {
        await api.updateEventSubscription(editingSub.id, data);
        setEditingSub(null);
        loadSubscriptions();
    };

    const handleDelete = async (id) => {
        if (!confirm('Delete this subscription?')) return;
        await api.deleteEventSubscription(id);
        loadSubscriptions();
    };

    const handleTest = async (id) => {
        try {
            await api.testEventSubscription(id);
            loadDeliveries(id);
        } catch { /* ignore */ }
    };

    const loadDeliveries = async (id) => {
        try {
            const data = await api.getEventDeliveries(id);
            setDeliveries(prev => ({ ...prev, [id]: data.deliveries || [] }));
        } catch { /* ignore */ }
    };

    const toggleExpand = (id) => {
        if (expandedId === id) {
            setExpandedId(null);
        } else {
            setExpandedId(id);
            if (!deliveries[id]) loadDeliveries(id);
        }
    };

    return (
        <div className="settings-card">
            <div className="settings-card__header">
                <div className="settings-card__header-left">
                    <Zap size={20} />
                    <div>
                        <h3>Webhook Subscriptions</h3>
                        <p>Receive HTTP notifications when events occur</p>
                    </div>
                </div>
                <Button variant="default" size="sm" onClick={() => setShowModal(true)}>
                    <Plus size={14} /> Add Webhook
                </Button>
            </div>

            {loading ? (
                <div className="settings-card__loading">Loading...</div>
            ) : subscriptions.length === 0 ? (
                <div className="settings-card__empty">
                    <Zap size={24} />
                    <p>No webhook subscriptions. Create one to get notified of events.</p>
                </div>
            ) : (
                <div className="api-settings__webhooks">
                    {subscriptions.map(sub => (
                        <div key={sub.id} className="api-settings__webhook-card">
                            <div className="api-settings__webhook-header" onClick={() => toggleExpand(sub.id)}>
                                <div className="api-settings__webhook-info">
                                    <span className="api-settings__webhook-name">{sub.name}</span>
                                    <span className="api-settings__webhook-url">{sub.url}</span>
                                    <div className="api-settings__webhook-events">
                                        {sub.events.slice(0, 3).map(e => (
                                            <Badge key={e} variant="secondary">{e}</Badge>
                                        ))}
                                        {sub.events.length > 3 && (
                                            <Badge variant="secondary">+{sub.events.length - 3}</Badge>
                                        )}
                                    </div>
                                </div>
                                <div className="api-settings__webhook-controls">
                                    <Badge variant={sub.is_active ? 'success' : 'destructive'}>
                                        {sub.is_active ? 'Active' : 'Inactive'}
                                    </Badge>
                                    {expandedId === sub.id ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
                                </div>
                            </div>
                            {expandedId === sub.id && (
                                <div className="api-settings__webhook-details">
                                    <div className="api-settings__webhook-actions">
                                        <Button variant="outline" size="sm" onClick={() => handleTest(sub.id)}>
                                            <Send size={14} /> Test
                                        </Button>
                                        <Button variant="outline" size="sm" onClick={() => setEditingSub(sub)}>
                                            Edit
                                        </Button>
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            onClick={() => handleDelete(sub.id)}
                                            className="text-destructive hover:text-destructive"
                                        >
                                            <Trash2 size={14} /> Delete
                                        </Button>
                                        <Button variant="ghost" size="sm" onClick={() => loadDeliveries(sub.id)}>
                                            <RefreshCw size={14} />
                                        </Button>
                                    </div>
                                    {deliveries[sub.id] && (
                                        <div className="api-settings__deliveries">
                                            <h4>Recent Deliveries</h4>
                                            {deliveries[sub.id].length === 0 ? (
                                                <p className="api-settings__muted">No deliveries yet</p>
                                            ) : (
                                                <table className="api-settings__table api-settings__table--compact">
                                                    <thead>
                                                        <tr>
                                                            <th>Event</th>
                                                            <th>Status</th>
                                                            <th>HTTP</th>
                                                            <th>Duration</th>
                                                            <th>Time</th>
                                                        </tr>
                                                    </thead>
                                                    <tbody>
                                                        {deliveries[sub.id].slice(0, 10).map(d => (
                                                            <tr key={d.id}>
                                                                <td><code>{d.event_type}</code></td>
                                                                <td>
                                                                    <Badge variant={d.status === 'success' ? 'success' : d.status === 'failed' ? 'destructive' : 'warning'}>
                                                                        {d.status}
                                                                    </Badge>
                                                                </td>
                                                                <td>{d.http_status || '-'}</td>
                                                                <td>{d.duration_ms ? `${d.duration_ms}ms` : '-'}</td>
                                                                <td className="api-settings__muted">
                                                                    {d.created_at ? new Date(d.created_at).toLocaleString() : '-'}
                                                                </td>
                                                            </tr>
                                                        ))}
                                                    </tbody>
                                                </table>
                                            )}
                                        </div>
                                    )}
                                </div>
                            )}
                        </div>
                    ))}
                </div>
            )}

            {showModal && (
                <WebhookSubscriptionModal
                    onClose={() => setShowModal(false)}
                    onSubmit={handleCreate}
                />
            )}
            {editingSub && (
                <WebhookSubscriptionModal
                    subscription={editingSub}
                    onClose={() => setEditingSub(null)}
                    onSubmit={handleUpdate}
                />
            )}
        </div>
    );
};

// ─── Analytics Section ─────────────────────────────────
const AnalyticsSection = () => {
    const [overview, setOverview] = useState(null);
    const [endpoints, setEndpoints] = useState([]);
    const [timeseries, setTimeseries] = useState([]);
    const [period, setPeriod] = useState('24h');
    const [loading, setLoading] = useState(true);

    const loadData = useCallback(() => {
        setLoading(true);
        Promise.all([
            api.getApiAnalyticsOverview(period),
            api.getApiAnalyticsEndpoints(period),
            api.getApiAnalyticsTimeseries(period),
        ]).then(([ov, ep, ts]) => {
            setOverview(ov);
            setEndpoints(ep.endpoints || []);
            setTimeseries(ts.data || []);
        }).catch(() => {}).finally(() => setLoading(false));
    }, [period]);

    useEffect(() => { loadData(); }, [loadData]);

    const maxCount = Math.max(...timeseries.map(d => d.count), 1);

    return (
        <div className="settings-card">
            <div className="settings-card__header">
                <div className="settings-card__header-left">
                    <BarChart3 size={20} />
                    <div>
                        <h3>API Usage Analytics</h3>
                        <p>Monitor API traffic, response times, and errors</p>
                    </div>
                </div>
                <div className="api-settings__period-select">
                    {['1h', '24h', '7d', '30d'].map(p => (
                        <Button
                            key={p}
                            size="sm"
                            variant={period === p ? 'default' : 'ghost'}
                            onClick={() => setPeriod(p)}
                        >
                            {p}
                        </Button>
                    ))}
                </div>
            </div>

            {loading ? (
                <div className="settings-card__loading">Loading...</div>
            ) : (
                <>
                    {overview && (
                        <div className="api-settings__stats-grid">
                            <div className="api-settings__stat-card">
                                <span className="api-settings__stat-value">{overview.total_requests.toLocaleString()}</span>
                                <span className="api-settings__stat-label">Total Requests</span>
                            </div>
                            <div className="api-settings__stat-card">
                                <span className="api-settings__stat-value">{overview.avg_response_time_ms}ms</span>
                                <span className="api-settings__stat-label">Avg Response Time</span>
                            </div>
                            <div className="api-settings__stat-card">
                                <span className="api-settings__stat-value">{overview.error_rate}%</span>
                                <span className="api-settings__stat-label">Error Rate</span>
                            </div>
                            <div className="api-settings__stat-card">
                                <span className="api-settings__stat-value">{overview.success_count.toLocaleString()}</span>
                                <span className="api-settings__stat-label">Successful</span>
                            </div>
                        </div>
                    )}

                    {timeseries.length > 0 && (
                        <div className="api-settings__chart">
                            <h4>Request Volume</h4>
                            <div className="api-settings__bar-chart">
                                {timeseries.map((d, i) => (
                                    <div key={i} className="api-settings__bar-col" title={`${d.period}: ${d.count} requests`}>
                                        <div
                                            className="api-settings__bar"
                                            style={{ height: `${(d.count / maxCount) * 100}%` }}
                                        />
                                        {d.errors > 0 && (
                                            <div
                                                className="api-settings__bar api-settings__bar--error"
                                                style={{ height: `${(d.errors / maxCount) * 100}%` }}
                                            />
                                        )}
                                    </div>
                                ))}
                            </div>
                        </div>
                    )}

                    {endpoints.length > 0 && (
                        <div className="api-settings__top-endpoints">
                            <h4>Top Endpoints</h4>
                            <table className="api-settings__table api-settings__table--compact">
                                <thead>
                                    <tr>
                                        <th>Method</th>
                                        <th>Endpoint</th>
                                        <th>Requests</th>
                                        <th>Avg Time</th>
                                        <th>Errors</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {endpoints.slice(0, 10).map((ep, i) => (
                                        <tr key={i}>
                                            <td><Badge variant="outline">{ep.method}</Badge></td>
                                            <td><code>{ep.endpoint}</code></td>
                                            <td>{ep.count.toLocaleString()}</td>
                                            <td>{ep.avg_response_time_ms}ms</td>
                                            <td>{ep.error_count > 0 ? <span className="api-settings__error-count">{ep.error_count}</span> : '-'}</td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                    )}
                </>
            )}
        </div>
    );
};

export default ApiSettingsTab;
