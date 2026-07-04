import { useState, useEffect, useCallback } from 'react';
import { Mail, Plus, Trash2, Check, Star } from 'lucide-react';
import api from '../services/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useToast } from '../contexts/ToastContext';

const FIELD_LABELS = {
    host: 'Host', port: 'Port', username: 'Username', password: 'Password', use_tls: 'Use TLS',
    api_key: 'API key', server_token: 'Server token', access_key: 'Access key ID',
    secret_key: 'Secret access key', region: 'Region', domain: 'Domain',
};

function Field({ label, children }) {
    return (
        <div className="sk-eprov__field">
            <Label>{label}</Label>
            {children}
        </div>
    );
}

export default function EmailProviders() {
    const toast = useToast();
    const [providers, setProviders] = useState([]);
    const [supported, setSupported] = useState({});
    const [loading, setLoading] = useState(true);
    const [type, setType] = useState('');
    const [form, setForm] = useState({});
    const [busy, setBusy] = useState(false);
    const [confirmId, setConfirmId] = useState(null);

    const load = useCallback(async () => {
        try {
            const data = await api.getEmailProviders();
            setProviders(data.providers || []);
            setSupported(data.supported || {});
        } catch {
            // leave state as-is
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => { load(); }, [load]);

    const spec = supported[type];
    const adding = Boolean(type);

    const startAdd = (key) => { setType(key); setForm({ from_name: 'ServerKit', use_tls: true }); };
    const cancel = () => { setType(''); setForm({}); };
    const onField = (key, value) => setForm((f) => ({ ...f, [key]: value }));

    const submit = async () => {
        setBusy(true);
        try {
            const payload = {
                provider: type,
                name: form.name || spec.name,
                from_address: form.from_address,
                from_name: form.from_name,
            };
            (spec.fields || []).forEach((f) => { if (form[f] !== undefined) payload[f] = form[f]; });
            const res = await api.addEmailProvider(payload);
            if (res?.test && res.test.success === false) {
                toast.warning(`Added, but test failed: ${res.test.error || 'check credentials'}`);
            } else {
                toast.success('Email provider added');
            }
            cancel();
            load();
        } catch (e) {
            toast.error(e?.message || 'Failed to add provider');
        } finally {
            setBusy(false);
        }
    };

    const onTest = async (id) => {
        setBusy(true);
        try {
            const r = await api.testEmailProvider(id);
            if (r.success) toast.success('Credentials OK'); else toast.error(r.error || 'Test failed');
            load();
        } catch {
            toast.error('Test failed');
        } finally {
            setBusy(false);
        }
    };

    const onDefault = async (id) => {
        try { await api.setDefaultEmailProvider(id); toast.success('Default transport updated'); load(); }
        catch { toast.error('Failed to set default'); }
    };

    const onDelete = async (id) => {
        try { await api.deleteEmailProvider(id); toast.success('Provider removed'); setConfirmId(null); load(); }
        catch { toast.error('Failed to remove'); }
    };

    return (
        <section className="sk-eprov">
            <header className="sk-eprov__head">
                <span className="sk-eprov__title"><Mail size={16} /> Email transport</span>
                <span className="sk-eprov__sub">How notification emails are sent</span>
            </header>

            {loading ? (
                <div className="sk-eprov__empty">Loading…</div>
            ) : (
                <>
                    {providers.length === 0 ? (
                        <div className="sk-eprov__empty">
                            No provider configured — emails fall back to the SMTP channel settings.
                        </div>
                    ) : (
                        <ul className="sk-eprov__list">
                            {providers.map((p) => (
                                <li key={p.id} className="sk-eprov__item">
                                    <div className="sk-eprov__meta">
                                        <span className="sk-eprov__name">
                                            {p.name}
                                            {p.is_default && <span className="sk-eprov__badge">Default</span>}
                                        </span>
                                        <span className="sk-eprov__type">
                                            {p.provider}
                                            {p.from_address ? ` · ${p.from_address}` : ''}
                                            {p.last_test_ok === true ? ' · ✓ tested' : ''}
                                            {p.last_test_ok === false ? ' · ✗ test failed' : ''}
                                        </span>
                                    </div>
                                    <div className="sk-eprov__actions">
                                        {!p.is_default && (
                                            <Button variant="ghost" size="sm" onClick={() => onDefault(p.id)}>
                                                <Star size={14} /> Default
                                            </Button>
                                        )}
                                        <Button variant="ghost" size="sm" disabled={busy} onClick={() => onTest(p.id)}>
                                            <Check size={14} /> Test
                                        </Button>
                                        {confirmId === p.id ? (
                                            <Button variant="destructive" size="sm" onClick={() => onDelete(p.id)}>Confirm</Button>
                                        ) : (
                                            <Button variant="ghost" size="sm" onClick={() => setConfirmId(p.id)} aria-label="Remove">
                                                <Trash2 size={14} />
                                            </Button>
                                        )}
                                    </div>
                                </li>
                            ))}
                        </ul>
                    )}

                    {!adding ? (
                        <div className="sk-eprov__add">
                            {Object.entries(supported).map(([key, s]) => (
                                <Button key={key} variant="outline" size="sm" onClick={() => startAdd(key)}>
                                    <Plus size={14} /> {s.name}
                                </Button>
                            ))}
                        </div>
                    ) : (
                        <div className="sk-eprov__form">
                            <div className="sk-eprov__form-title">Add {spec?.name}</div>
                            <div className="sk-eprov__grid">
                                <Field label="Display name">
                                    <Input value={form.name || ''} onChange={(e) => onField('name', e.target.value)} placeholder={spec?.name} />
                                </Field>
                                <Field label="From address">
                                    <Input value={form.from_address || ''} onChange={(e) => onField('from_address', e.target.value)} placeholder="no-reply@yourdomain.com" />
                                </Field>
                                <Field label="From name">
                                    <Input value={form.from_name || ''} onChange={(e) => onField('from_name', e.target.value)} />
                                </Field>
                                {(spec?.fields || []).map((f) => (
                                    f === 'use_tls' ? (
                                        <Field key={f} label={FIELD_LABELS[f] || f}>
                                            <label className="sk-eprov__check">
                                                <input
                                                    type="checkbox"
                                                    checked={form.use_tls !== false}
                                                    onChange={(e) => onField('use_tls', e.target.checked)}
                                                />
                                                STARTTLS
                                            </label>
                                        </Field>
                                    ) : (
                                        <Field key={f} label={FIELD_LABELS[f] || f}>
                                            <Input
                                                type={spec.secrets.includes(f) ? 'password' : (f === 'port' ? 'number' : 'text')}
                                                value={form[f] || ''}
                                                onChange={(e) => onField(f, e.target.value)}
                                            />
                                        </Field>
                                    )
                                ))}
                            </div>
                            <div className="sk-eprov__form-actions">
                                <Button variant="outline" size="sm" onClick={cancel} disabled={busy}>Cancel</Button>
                                <Button size="sm" onClick={submit} disabled={busy}>{busy ? 'Adding…' : 'Add & test'}</Button>
                            </div>
                        </div>
                    )}
                </>
            )}
        </section>
    );
}
