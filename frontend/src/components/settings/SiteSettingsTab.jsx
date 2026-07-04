import { useState, useEffect } from 'react';
import api from '../../services/api';
import { Switch } from '@/components/ui/switch';
import { Label } from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Pill } from '@/components/ds';

const SiteSettingsTab = ({ onDevModeChange }) => {
    const [settings, setSettings] = useState({
        registration_enabled: false,
        dev_mode: false
    });
    const [basePort, setBasePort] = useState('0');
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [savingPort, setSavingPort] = useState(false);
    const [message, setMessage] = useState(null);
    const [https, setHttps] = useState({ base_domain: '', server_ip: '', https_enabled: false, dns_mode: 'wildcard', providers: [], bases: [] });
    const [baseDomain, setBaseDomain] = useState('');
    const [serverIp, setServerIp] = useState('');
    const [providerId, setProviderId] = useState('');
    const [savingDomain, setSavingDomain] = useState(false);
    const [savingDnsMode, setSavingDnsMode] = useState(false);
    // Base-domain registry: add a new base + track which row an action is running on.
    const [newDomain, setNewDomain] = useState('');
    const [newDnsMode, setNewDnsMode] = useState('wildcard');
    const [addingDomain, setAddingDomain] = useState(false);
    const [rowBusy, setRowBusy] = useState('');   // domain currently being mutated

    useEffect(() => {
        loadSettings();
    }, []);

    async function loadSettings() {
        try {
            const data = await api.getSystemSettings();
            setSettings({
                registration_enabled: data.registration_enabled || false,
                dev_mode: data.dev_mode || false
            });
            setBasePort(String(data.managed_app_base_port ?? 0));
            await loadHttps();
        } catch (err) {
            console.error('Failed to load settings:', err);
        } finally {
            setLoading(false);
        }
    }

    async function loadHttps() {
        try {
            const h = await api.getSitesHttpsStatus();
            setHttps(h);
            setBaseDomain(h.base_domain || '');
            setServerIp(h.server_ip || '');
            if (h.providers?.length && !providerId) setProviderId(String(h.providers[0].id));
        } catch { /* non-admin or endpoint unavailable */ }
    }

    // The bases to show: the registry when populated, else a synthetic row for
    // the single legacy base (editable in place) so single-domain installs and
    // fresh installs both work through the same UI.
    const displayBases = (https.bases && https.bases.length)
        ? https.bases
        : (baseDomain
            ? [{ domain: baseDomain, is_default: true, https_enabled: https.https_enabled, dns_mode: https.dns_mode, _legacy: true }]
            : []);
    const hasRegistry = !!(https.bases && https.bases.length);

    async function handleAddDomain() {
        const domain = newDomain.trim();
        if (!domain) return;
        setAddingDomain(true);
        setMessage(null);
        try {
            const res = await api.addSiteBaseDomain(domain, { dnsMode: newDnsMode });
            if (res.success) {
                setNewDomain('');
                setMessage({ type: 'success', text: `Added base domain ${domain}` });
                await loadHttps();
            } else {
                setMessage({ type: 'error', text: res.error || 'Could not add base domain' });
            }
        } catch (err) {
            setMessage({ type: 'error', text: err.message || 'Could not add base domain' });
        } finally {
            setAddingDomain(false);
        }
    }

    async function handleRemoveDomain(domain) {
        setRowBusy(domain);
        setMessage(null);
        try {
            const res = await api.removeSiteBaseDomain(domain);
            if (res.success) {
                setMessage({ type: 'success', text: `Removed ${domain}` });
                await loadHttps();
            } else {
                setMessage({ type: 'error', text: res.error || 'Could not remove base domain' });
            }
        } catch (err) {
            setMessage({ type: 'error', text: err.message || 'Could not remove base domain' });
        } finally {
            setRowBusy('');
        }
    }

    async function handleMakeDefault(domain) {
        setRowBusy(domain);
        setMessage(null);
        try {
            const res = await api.setDefaultSiteBaseDomain(domain);
            if (res.success) {
                setMessage({ type: 'success', text: `${domain} is now the default base domain` });
                await loadHttps();
            } else {
                setMessage({ type: 'error', text: res.error || 'Could not set default' });
            }
        } catch (err) {
            setMessage({ type: 'error', text: err.message || 'Could not set default' });
        } finally {
            setRowBusy('');
        }
    }

    async function handleRowDnsMode(base, mode) {
        if (base._legacy) return handleSaveDnsMode(mode);   // legacy setting path
        setRowBusy(base.domain);
        setMessage(null);
        try {
            const res = await api.updateSiteBaseDomain(base.domain, { dnsMode: mode });
            if (res.success) await loadHttps();
            else setMessage({ type: 'error', text: res.error || 'Could not update DNS mode' });
        } catch (err) {
            setMessage({ type: 'error', text: err.message || 'Could not update DNS mode' });
        } finally {
            setRowBusy('');
        }
    }

    async function handleSetupHttpsFor(base) {
        if (!providerId) {
            setMessage({ type: 'error', text: 'Connect and select a DNS provider first' });
            return;
        }
        setRowBusy(base.domain);
        setMessage(null);
        try {
            if (serverIp.trim()) await api.updateSystemSetting('server_public_ip', serverIp.trim());
            // A legacy synthetic row has no registry entry yet — omit base so the
            // backend targets the default (and persists to settings).
            const res = await api.setupSitesHttps(Number(providerId), undefined, base._legacy ? undefined : base.domain);
            if (res.success) {
                setMessage({ type: 'success', text: `Wildcard HTTPS set up for *.${res.base_domain}` + (res.warning ? ` — ${res.warning}` : '') });
                await loadHttps();
            } else {
                setMessage({ type: 'error', text: res.error || 'HTTPS setup failed' });
            }
        } catch (err) {
            setMessage({ type: 'error', text: err.message || 'HTTPS setup failed' });
        } finally {
            setRowBusy('');
        }
    }

    async function handleSaveServerIp() {
        setSavingDomain(true);
        setMessage(null);
        try {
            await api.updateSystemSetting('server_public_ip', serverIp.trim());
            setMessage({ type: 'success', text: 'Server public IP saved' });
        } catch (err) {
            setMessage({ type: 'error', text: err.message || 'Failed to save server IP' });
        } finally {
            setSavingDomain(false);
        }
    }

    // Legacy single-base edit: persists the sites_base_domain setting in place
    // (only used for the synthetic row on installs with no registry entries yet).
    async function handleSaveDomain() {
        setSavingDomain(true);
        setMessage(null);
        try {
            await api.updateSystemSetting('sites_base_domain', baseDomain.trim());
            setMessage({ type: 'success', text: 'Base domain saved' });
            await loadHttps();
        } catch (err) {
            setMessage({ type: 'error', text: err.message || 'Failed to save base domain' });
        } finally {
            setSavingDomain(false);
        }
    }

    async function handleSaveDnsMode(mode) {
        setSavingDnsMode(true);
        setMessage(null);
        try {
            await api.updateSystemSetting('sites_dns_mode', mode);
            setHttps((h) => ({ ...h, dns_mode: mode }));
            setMessage({
                type: 'success',
                text: mode === 'per-site'
                    ? 'Per-site DNS — new sites get their own A record'
                    : 'Wildcard DNS — new sites ride the *.base record',
            });
        } catch (err) {
            setMessage({ type: 'error', text: err.message || 'Failed to update DNS mode' });
        } finally {
            setSavingDnsMode(false);
        }
    }

    async function handleSaveBasePort() {
        const value = parseInt(basePort, 10);
        if (Number.isNaN(value) || value < 0 || value > 65535) {
            setMessage({ type: 'error', text: 'Base port must be between 0 and 65535 (0 = template default)' });
            return;
        }
        setSavingPort(true);
        setMessage(null);
        try {
            await api.updateSystemSetting('managed_app_base_port', value);
            setBasePort(String(value));
            setMessage({
                type: 'success',
                text: value === 0
                    ? 'Base port reset — new apps use each template\'s default'
                    : `New apps will be assigned ports starting from ${value}`
            });
        } catch (err) {
            setMessage({ type: 'error', text: err.message || 'Failed to update base port' });
        } finally {
            setSavingPort(false);
        }
    }

    async function handleToggleSetting(key, label) {
        setSaving(true);
        setMessage(null);

        try {
            const newValue = !settings[key];
            await api.updateSystemSetting(key, newValue);
            setSettings({ ...settings, [key]: newValue });
            setMessage({ type: 'success', text: `${label} ${newValue ? 'enabled' : 'disabled'}` });
            if (key === 'dev_mode' && onDevModeChange) {
                onDevModeChange(newValue);
            }
        } catch (err) {
            setMessage({ type: 'error', text: err.message || 'Failed to update setting' });
        } finally {
            setSaving(false);
        }
    }

    if (loading) {
        return <div className="settings-section"><p>Loading...</p></div>;
    }

    return (
        <div className="settings-section">
            <h2>Site Settings</h2>
            <p className="section-description">Configure global site settings</p>

            {message && (
                <div className={`message ${message.type}`}>{message.text}</div>
            )}

            <div className="settings-card">
                <h3>User Registration</h3>
                <p>Allow new users to create accounts on the login page.</p>

                <div className="form-group">
                    <div className="settings-row">
                        <div className="settings-label">
                            <Label>Enable public registration</Label>
                        </div>
                        <Switch
                            checked={settings.registration_enabled}
                            onCheckedChange={() => handleToggleSetting('registration_enabled', 'User registration')}
                            disabled={saving}
                        />
                    </div>
                    <span className="form-help">
                        When disabled, only administrators can create new user accounts.
                    </span>
                </div>
            </div>

            <div className="settings-card">
                <h3>Managed App Ports</h3>
                <p>Control the host port assigned to new WordPress sites and other managed apps.</p>

                <div className="form-group">
                    <div className="settings-row">
                        <div className="settings-label">
                            <Label htmlFor="managed-app-base-port">Base port</Label>
                        </div>
                        <div className="settings-control">
                            <Input
                                id="managed-app-base-port"
                                type="number"
                                min={0}
                                max={65535}
                                value={basePort}
                                onChange={(e) => setBasePort(e.target.value)}
                                disabled={savingPort}
                                className="w-32"
                            />
                            <Button onClick={handleSaveBasePort} disabled={savingPort}>
                                {savingPort ? 'Saving…' : 'Save'}
                            </Button>
                        </div>
                    </div>
                    <span className="form-help">
                        New apps get the first free port at or above this number. Set to <strong>0</strong> to
                        use each template&apos;s own default (WordPress starts at 8300). Ports already in use are
                        always skipped, so collisions can&apos;t happen.
                    </span>
                </div>
            </div>

            <div className="settings-card">
                <h3>Managed Sites — Base Domains</h3>
                <p>Publish managed sites at <code>&lt;name&gt;.&lt;base-domain&gt;</code>. Register one or more base domains; a new site can be created under any of them, defaulting to the one marked <strong>Default</strong>. Point a wildcard record <code>*.&lt;base&gt;</code> (or per-site A records) at this server.</p>

                <div className="form-group">
                    <div className="settings-row">
                        <div className="settings-label">
                            <Label htmlFor="sites-server-ip">Server public IP</Label>
                        </div>
                        <div className="settings-control">
                            <Input
                                id="sites-server-ip"
                                type="text"
                                placeholder="203.0.113.10"
                                value={serverIp}
                                onChange={(e) => setServerIp(e.target.value)}
                                className="w-56"
                            />
                            <Button onClick={handleSaveServerIp} disabled={savingDomain}>
                                {savingDomain ? 'Saving…' : 'Save'}
                            </Button>
                        </div>
                    </div>
                    <span className="form-help">Shared by every base domain — used to auto-create their DNS A records.</span>
                </div>

                {https.providers?.length > 0 ? (
                    <div className="form-group">
                        <div className="settings-row">
                            <div className="settings-label"><Label>DNS provider for HTTPS</Label></div>
                            <div className="settings-control">
                                <select className="settings-select" value={providerId} onChange={(e) => setProviderId(e.target.value)}>
                                    {https.providers.map((p) => (
                                        <option key={p.id} value={p.id}>{p.name} ({p.provider})</option>
                                    ))}
                                </select>
                            </div>
                        </div>
                        <span className="form-help">Used to issue each base&apos;s wildcard certificate (DNS-01). Each base can use a different connected provider.</span>
                    </div>
                ) : (
                    <span className="form-help">Connect a DNS provider under Email → DNS Providers to enable per-domain wildcard HTTPS.</span>
                )}

                {displayBases.length === 0 ? (
                    <p className="form-help">No base domain yet — add one below to start publishing sites at real subdomains.</p>
                ) : displayBases.map((b) => (
                    <div key={b.domain} className="form-group">
                        <div className="settings-row">
                            <div className="settings-label">
                                {b._legacy ? (
                                    <div className="settings-control">
                                        <Input type="text" placeholder="apps.example.com" value={baseDomain}
                                            onChange={(e) => setBaseDomain(e.target.value)} className="w-56" />
                                        <Button onClick={handleSaveDomain} disabled={savingDomain}>
                                            {savingDomain ? 'Saving…' : 'Save'}
                                        </Button>
                                    </div>
                                ) : (
                                    <span className="flex items-center gap-2">
                                        <code>{b.domain}</code>
                                        {b.is_default && <Pill kind="blue" dot={false}>Default</Pill>}
                                        <Pill kind={b.https_enabled ? 'green' : 'gray'} dot={false}>
                                            {b.https_enabled ? 'HTTPS' : 'HTTP only'}
                                        </Pill>
                                    </span>
                                )}
                            </div>
                            <div className="settings-control">
                                <select className="settings-select" value={b.dns_mode || 'wildcard'}
                                    onChange={(e) => handleRowDnsMode(b, e.target.value)}
                                    disabled={rowBusy === b.domain || savingDnsMode}>
                                    <option value="wildcard">Wildcard DNS</option>
                                    <option value="per-site">Per-site DNS</option>
                                </select>
                                <Button variant="outline" onClick={() => handleSetupHttpsFor(b)}
                                    disabled={rowBusy === b.domain || !https.providers?.length}>
                                    {rowBusy === b.domain ? 'Working…' : (b.https_enabled ? 'Renew HTTPS' : 'Set up HTTPS')}
                                </Button>
                                {hasRegistry && !b.is_default && (
                                    <Button variant="ghost" onClick={() => handleMakeDefault(b.domain)} disabled={rowBusy === b.domain}>
                                        Make default
                                    </Button>
                                )}
                                {hasRegistry && displayBases.length > 1 && (
                                    <Button variant="ghost" onClick={() => handleRemoveDomain(b.domain)} disabled={rowBusy === b.domain}>
                                        Remove
                                    </Button>
                                )}
                            </div>
                        </div>
                        <span className="form-help">
                            Sites publish at <code>&lt;name&gt;.{b.domain || 'base-domain'}</code>.{' '}
                            {(b.dns_mode || 'wildcard') === 'wildcard'
                                ? <>Point <code>*.{b.domain || 'base-domain'}</code> at this server.</>
                                : <>Each new site gets its own A record, auto-created via the provider.</>}
                        </span>
                    </div>
                ))}

                <div className="form-group">
                    <div className="settings-row">
                        <div className="settings-label"><Label htmlFor="new-base-domain">Add base domain</Label></div>
                        <div className="settings-control">
                            <Input id="new-base-domain" type="text" placeholder="toto.com"
                                value={newDomain} onChange={(e) => setNewDomain(e.target.value)} className="w-56" />
                            <select className="settings-select" value={newDnsMode} onChange={(e) => setNewDnsMode(e.target.value)}>
                                <option value="wildcard">Wildcard DNS</option>
                                <option value="per-site">Per-site DNS</option>
                            </select>
                            <Button onClick={handleAddDomain} disabled={addingDomain || !newDomain.trim()}>
                                {addingDomain ? 'Adding…' : 'Add'}
                            </Button>
                        </div>
                    </div>
                    <span className="form-help">Register another domain sites can be published under. Set up its wildcard HTTPS from its row above.</span>
                </div>
            </div>

            <div className="settings-card">
                <h3>Developer Mode</h3>
                <p>Enable developer tools and diagnostics.</p>

                <div className="form-group">
                    <div className="settings-row">
                        <div className="settings-label">
                            <Label>Enable developer mode</Label>
                        </div>
                        <Switch
                            checked={settings.dev_mode}
                            onCheckedChange={() => handleToggleSetting('dev_mode', 'Developer mode')}
                            disabled={saving}
                        />
                    </div>
                    <span className="form-help">
                        Enables the Developer tab with icon reference and diagnostic tools.
                    </span>
                </div>
            </div>
        </div>
    );
};

export default SiteSettingsTab;
