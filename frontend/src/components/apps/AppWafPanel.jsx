import { useState, useEffect, useCallback } from 'react';
import { ShieldCheck, ListChecks, X, RefreshCw } from 'lucide-react';
import api from '../../services/api';
import { useToast } from '../../contexts/ToastContext';
import { Pill, SegControl } from '../ds';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

// WAF modes with one-line descriptions shown under the selector.
const MODE_OPTIONS = [
    { value: 'off', label: 'Off' },
    { value: 'detect', label: 'Detect-only' },
    { value: 'block', label: 'Block' },
];

const MODE_HINTS = {
    off: 'ModSecurity is disabled for this app — no rules are evaluated.',
    detect: 'Rules are evaluated and matches are logged, but requests are never blocked.',
    block: 'Malicious requests that exceed the anomaly threshold are rejected.',
};

const MODE_PILL = {
    off: 'gray',
    detect: 'amber',
    block: 'green',
};

// ModSecurity severities → pill colors (best-effort; tolerate unknown values).
const SEVERITY_PILL = {
    critical: 'red',
    emergency: 'red',
    alert: 'red',
    error: 'red',
    warning: 'amber',
    notice: 'amber',
    info: 'cyan',
    debug: 'gray',
};

function severityKind(severity) {
    if (!severity) return 'gray';
    return SEVERITY_PILL[String(severity).toLowerCase()] || 'gray';
}

const DEFAULT_POLICY = {
    mode: 'off',
    paranoia_level: 1,
    anomaly_threshold: 5,
    disabled_rule_ids: [],
};

const AppWafPanel = ({ app, onChanged }) => {
    const toast = useToast();

    const [policy, setPolicy] = useState(DEFAULT_POLICY);
    const [status, setStatus] = useState(null);
    const [loading, setLoading] = useState(true);

    const [installing, setInstalling] = useState(false);
    const [saving, setSaving] = useState(false);
    const [applying, setApplying] = useState(false);

    const [ruleInput, setRuleInput] = useState('');

    const [events, setEvents] = useState([]);
    const [eventsLoading, setEventsLoading] = useState(true);

    const loadPolicy = useCallback(async () => {
        try {
            const data = await api.getWafPolicy(app.id);
            const merged = { ...DEFAULT_POLICY, ...(data || {}) };
            // Normalize disabled rule ids to a clean numeric array.
            merged.disabled_rule_ids = Array.isArray(merged.disabled_rule_ids)
                ? merged.disabled_rule_ids.map(Number).filter((n) => Number.isFinite(n))
                : [];
            setPolicy(merged);
        } catch (err) {
            console.error('Failed to load WAF policy:', err);
        }
    }, [app.id]);

    const loadStatus = useCallback(async () => {
        try {
            const data = await api.getWafStatus();
            setStatus(data);
        } catch (err) {
            console.error('Failed to load WAF status:', err);
        }
    }, []);

    const loadEvents = useCallback(async () => {
        setEventsLoading(true);
        try {
            const data = await api.getWafEvents(app.id, 50);
            setEvents(Array.isArray(data) ? data : (data?.events || []));
        } catch (err) {
            console.error('Failed to load WAF events:', err);
            setEvents([]);
        } finally {
            setEventsLoading(false);
        }
    }, [app.id]);

    useEffect(() => {
        let active = true;
        (async () => {
            setLoading(true);
            await Promise.all([loadPolicy(), loadStatus()]);
            if (active) setLoading(false);
        })();
        loadEvents();
        return () => { active = false; };
    }, [loadPolicy, loadStatus, loadEvents]);

    function setField(key, value) {
        setPolicy((prev) => ({ ...prev, [key]: value }));
    }

    async function handleInstall() {
        setInstalling(true);
        try {
            const result = await api.installWaf();
            if (result?.success === false) {
                toast.error(result.message || result.error || 'Failed to install ModSecurity');
            } else {
                toast.success(result?.message || 'ModSecurity installed.');
            }
            await loadStatus();
        } catch (err) {
            toast.error(err.message || 'Failed to install ModSecurity');
        } finally {
            setInstalling(false);
        }
    }

    function handleAddRule() {
        const id = parseInt(ruleInput, 10);
        if (!Number.isFinite(id)) {
            setRuleInput('');
            return;
        }
        setPolicy((prev) => {
            if (prev.disabled_rule_ids.includes(id)) return prev;
            return { ...prev, disabled_rule_ids: [...prev.disabled_rule_ids, id] };
        });
        setRuleInput('');
    }

    function handleRuleKeyDown(e) {
        if (e.key === 'Enter') {
            e.preventDefault();
            handleAddRule();
        }
    }

    function handleRemoveRule(id) {
        setField('disabled_rule_ids', policy.disabled_rule_ids.filter((r) => r !== id));
    }

    function buildPayload() {
        return {
            mode: policy.mode,
            paranoia_level: parseInt(policy.paranoia_level, 10) || 1,
            anomaly_threshold: parseInt(policy.anomaly_threshold, 10) || 0,
            disabled_rule_ids: policy.disabled_rule_ids,
        };
    }

    async function handleSave() {
        setSaving(true);
        try {
            const payload = buildPayload();
            const data = await api.updateWafPolicy(app.id, payload);
            if (data) {
                const merged = { ...DEFAULT_POLICY, ...data };
                merged.disabled_rule_ids = Array.isArray(merged.disabled_rule_ids)
                    ? merged.disabled_rule_ids.map(Number).filter((n) => Number.isFinite(n))
                    : payload.disabled_rule_ids;
                setPolicy(merged);
            }
            toast.success('WAF policy saved.');
            onChanged?.();
        } catch (err) {
            toast.error(err.message || 'Failed to save WAF policy');
        } finally {
            setSaving(false);
        }
    }

    async function handleApply() {
        setApplying(true);
        try {
            const result = await api.applyWaf(app.id);
            if (result?.success === false) {
                toast.error(result.message || result.error || 'Failed to apply WAF rules');
            } else if (result?.manual_include) {
                toast.warning(
                    `Rules written, but no vhost was found. Include it manually: ${result.manual_include}`,
                    12000,
                );
            } else {
                toast.success(result?.message || 'WAF rules applied and nginx reloaded.');
            }
        } catch (err) {
            toast.error(err.message || 'Failed to apply WAF rules');
        } finally {
            setApplying(false);
        }
    }

    const installed = status?.installed;
    const modeHint = MODE_HINTS[policy.mode] || '';

    return (
        <div className="waf-panel">
            {/* Install banner — only when ModSecurity is reported absent */}
            {status && !installed && (
                <div className="waf-panel__banner">
                    <div className="waf-panel__banner-text">
                        <strong>ModSecurity is not installed on this server</strong>
                        <span>
                            Install ModSecurity and the OWASP Core Rule Set to enable the web
                            application firewall for this app.
                        </span>
                    </div>
                    <Button size="sm" onClick={handleInstall} disabled={installing}>
                        {installing ? 'Installing…' : 'Install'}
                    </Button>
                </div>
            )}

            {/* Policy controls */}
            <div className="app-panel waf-panel__section">
                <div className="app-panel-header">
                    <ShieldCheck />
                    <span>Firewall Policy</span>
                    <span className="app-panel-header-actions">
                        {!loading && (
                            <Pill kind={MODE_PILL[policy.mode] || 'gray'}>
                                {MODE_OPTIONS.find((m) => m.value === policy.mode)?.label || 'Off'}
                            </Pill>
                        )}
                    </span>
                </div>
                <div className="app-panel-body">
                    <p className="app-panel-hint">
                        ModSecurity with the OWASP Core Rule Set inspects incoming requests for
                        common attacks (SQL injection, XSS, and more) before they reach this app.
                    </p>

                    <div className="waf-panel__mode">
                        <Label>Mode</Label>
                        <SegControl
                            options={MODE_OPTIONS}
                            value={policy.mode}
                            onChange={(v) => setField('mode', v)}
                        />
                        <span className="waf-panel__mode-hint">{modeHint}</span>
                    </div>

                    <div className="container-ops__grid">
                        <div className="container-ops__input">
                            <Label htmlFor={`waf-paranoia-${app.id}`}>Paranoia level (1–4)</Label>
                            <Input
                                id={`waf-paranoia-${app.id}`}
                                type="number"
                                min={1}
                                max={4}
                                value={policy.paranoia_level}
                                onChange={(e) => setField('paranoia_level', e.target.value)}
                                disabled={loading}
                            />
                        </div>
                        <div className="container-ops__input">
                            <Label htmlFor={`waf-threshold-${app.id}`}>Anomaly threshold</Label>
                            <Input
                                id={`waf-threshold-${app.id}`}
                                type="number"
                                min={0}
                                value={policy.anomaly_threshold}
                                onChange={(e) => setField('anomaly_threshold', e.target.value)}
                                disabled={loading}
                            />
                        </div>
                    </div>

                    <div className="waf-panel__rules">
                        <Label htmlFor={`waf-rule-${app.id}`}>Disabled CRS rule IDs</Label>
                        <span className="container-ops__field-hint">
                            Suppress specific Core Rule Set rules that cause false positives for
                            this app.
                        </span>
                        <div className="waf-panel__rules-input">
                            <Input
                                id={`waf-rule-${app.id}`}
                                type="number"
                                value={ruleInput}
                                onChange={(e) => setRuleInput(e.target.value)}
                                onKeyDown={handleRuleKeyDown}
                                placeholder="e.g. 942100"
                                disabled={loading}
                            />
                            <Button
                                variant="outline"
                                size="sm"
                                onClick={handleAddRule}
                                disabled={loading || !ruleInput.trim()}
                            >
                                Add
                            </Button>
                        </div>
                        {policy.disabled_rule_ids.length > 0 ? (
                            <div className="waf-panel__chips">
                                {policy.disabled_rule_ids.map((id) => (
                                    <span key={id} className="waf-panel__chip">
                                        <span className="mono">{id}</span>
                                        <button
                                            type="button"
                                            className="waf-panel__chip-remove"
                                            onClick={() => handleRemoveRule(id)}
                                            aria-label={`Remove rule ${id}`}
                                        >
                                            <X size={13} />
                                        </button>
                                    </span>
                                ))}
                            </div>
                        ) : (
                            <span className="container-ops__field-hint">No rules disabled.</span>
                        )}
                    </div>

                    <div className="app-detail-actions container-ops__actions">
                        <Button size="sm" onClick={handleSave} disabled={saving || loading}>
                            {saving ? 'Saving…' : 'Save policy'}
                        </Button>
                        <Button variant="outline" size="sm" onClick={handleApply} disabled={applying || loading}>
                            {applying ? 'Applying…' : 'Re-apply'}
                        </Button>
                    </div>
                </div>
            </div>

            {/* Recent events */}
            <div className="app-panel waf-panel__section">
                <div className="app-panel-header">
                    <ListChecks />
                    <span>Recent events</span>
                    <span className="app-panel-header-actions">
                        <Button variant="ghost" size="sm" onClick={loadEvents} disabled={eventsLoading}>
                            <RefreshCw size={14} />
                            {eventsLoading ? 'Loading…' : 'Refresh'}
                        </Button>
                    </span>
                </div>
                <div className="app-panel-body">
                    {events.length > 0 ? (
                        <div className="waf-panel__table-wrap">
                            <table className="waf-panel__table">
                                <thead>
                                    <tr>
                                        <th>Rule ID</th>
                                        <th>Severity</th>
                                        <th>Message</th>
                                        <th>URI</th>
                                        <th>Client</th>
                                        <th>Time</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {events.map((ev, i) => (
                                        <tr key={ev.id ?? `${ev.rule_id ?? 'rule'}-${i}`}>
                                            <td className="mono">{ev.rule_id ?? '—'}</td>
                                            <td>
                                                {ev.severity ? (
                                                    <Pill kind={severityKind(ev.severity)}>{ev.severity}</Pill>
                                                ) : '—'}
                                            </td>
                                            <td className="waf-panel__msg">{ev.message || '—'}</td>
                                            <td className="mono waf-panel__uri">{ev.uri || '—'}</td>
                                            <td className="mono">{ev.client_ip || ev.client || '—'}</td>
                                            <td className="waf-panel__time">
                                                {ev.timestamp ? new Date(ev.timestamp).toLocaleString() : '—'}
                                            </td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                    ) : (
                        <p className="app-panel-hint">
                            {eventsLoading ? 'Loading events…' : 'No WAF events recorded.'}
                        </p>
                    )}
                </div>
            </div>
        </div>
    );
};

export default AppWafPanel;
