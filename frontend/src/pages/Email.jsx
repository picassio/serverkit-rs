import { useState, useEffect, useCallback } from 'react';
import { Globe, AtSign, Forward, Server, Inbox, Mail, Send, KeyRound, ShieldAlert, ShieldCheck, AppWindow } from 'lucide-react';
import useTabParam from '../hooks/useTabParam';
import { api } from '../services/api';
import { useToast } from '../contexts/ToastContext';
import EmptyState from '../components/EmptyState';
import ConfirmDialog from '../components/ConfirmDialog';
import Modal from '@/components/Modal';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import { Pill, MetricCard, PageTopbar, DataTable } from '@/components/ds';

const VALID_TABS = ['status', 'domains', 'accounts', 'aliases', 'forwarding', 'dns-providers', 'spam', 'webmail', 'queue'];

const TAB_LABELS = {
    'dns-providers': 'DNS Providers',
    queue: 'Queue & Logs',
};

const SERVICE_ICONS = {
    postfix: Send,
    dovecot: Inbox,
    opendkim: KeyRound,
    spamassassin: ShieldAlert,
    roundcube: AppWindow,
};

// running → green / stopped → red / not installed → gray
function statusPill(data) {
    if (data?.running) return <Pill kind="green">running</Pill>;
    if (data?.installed) return <Pill kind="red">stopped</Pill>;
    return <Pill kind="gray">not installed</Pill>;
}

// DKIM/SPF/DMARC presence (record configured vs missing)
function dnsPill(value) {
    return value
        ? <Pill kind="green">set</Pill>
        : <Pill kind="amber">missing</Pill>;
}

function Email() {
    const [activeTab, setActiveTab] = useTabParam('/email', VALID_TABS);
    const [status, setStatus] = useState(null);
    const [loading, setLoading] = useState(true);
    const [actionLoading, setActionLoading] = useState(false);
    const [confirmDialog, setConfirmDialog] = useState(null);

    // Domains
    const [domains, setDomains] = useState([]);
    const [showDomainForm, setShowDomainForm] = useState(false);
    const [newDomain, setNewDomain] = useState({ name: '', dns_provider_id: '', dns_zone_id: '' });

    // Accounts
    const [selectedDomainId, setSelectedDomainId] = useState('');
    const [accounts, setAccounts] = useState([]);
    const [showAccountForm, setShowAccountForm] = useState(false);
    const [newAccount, setNewAccount] = useState({ username: '', password: '', quota_mb: 1024 });
    const [showPasswordModal, setShowPasswordModal] = useState(null);
    const [newPassword, setNewPassword] = useState('');

    // Aliases
    const [aliases, setAliases] = useState([]);
    const [showAliasForm, setShowAliasForm] = useState(false);
    const [newAlias, setNewAlias] = useState({ source: '', destination: '' });
    const [aliasDomainId, setAliasDomainId] = useState('');

    // Forwarding
    const [allAccounts, setAllAccounts] = useState([]);
    const [selectedAccountId, setSelectedAccountId] = useState('');
    const [forwardingRules, setForwardingRules] = useState([]);
    const [showForwardForm, setShowForwardForm] = useState(false);
    const [newForward, setNewForward] = useState({ destination: '', keep_copy: true });

    // DNS Providers
    const [providers, setProviders] = useState([]);
    const [showProviderForm, setShowProviderForm] = useState(false);
    const [newProvider, setNewProvider] = useState({ name: '', provider: 'cloudflare', api_key: '', api_secret: '', api_email: '', is_default: false });
    const [providerZones, setProviderZones] = useState({});

    // Spam
    const [spamConfig, setSpamConfig] = useState(null);

    // Webmail
    const [webmailStatus, setWebmailStatus] = useState(null);
    const [proxyDomain, setProxyDomain] = useState('');
    const [installHostname, setInstallHostname] = useState('');

    // Queue & Logs
    const [queue, setQueue] = useState([]);
    const [logs, setLogs] = useState([]);
    const [logLines, setLogLines] = useState(100);

    const toast = useToast();

    useEffect(() => { loadStatus(); }, []);

    const loadStatus = async () => {
        setLoading(true);
        try {
            const data = await api.getEmailStatus();
            setStatus(data);
        } catch (err) {
            console.error('Failed to load email status:', err);
        } finally {
            setLoading(false);
        }
    };

    const loadDomains = useCallback(async () => {
        try {
            const data = await api.getEmailDomains();
            setDomains(data.domains || []);
        } catch (err) { toast.error('Failed to load domains'); }
    }, []);

    useEffect(() => { if (activeTab === 'domains') loadDomains(); }, [activeTab]);

    const loadAccounts = useCallback(async (domainId) => {
        if (!domainId) return;
        try {
            const data = await api.getEmailAccounts(domainId);
            setAccounts(data.accounts || []);
        } catch (err) { toast.error('Failed to load accounts'); }
    }, []);

    useEffect(() => { if (activeTab === 'accounts' && selectedDomainId) loadAccounts(selectedDomainId); }, [activeTab, selectedDomainId]);
    useEffect(() => {
        if (activeTab === 'accounts' && domains.length === 0) loadDomains();
    }, [activeTab]);

    const loadAliases = useCallback(async (domainId) => {
        if (!domainId) return;
        try {
            const data = await api.getEmailAliases(domainId);
            setAliases(data.aliases || []);
        } catch (err) { toast.error('Failed to load aliases'); }
    }, []);

    useEffect(() => { if (activeTab === 'aliases' && aliasDomainId) loadAliases(aliasDomainId); }, [activeTab, aliasDomainId]);
    useEffect(() => {
        if (activeTab === 'aliases' && domains.length === 0) loadDomains();
    }, [activeTab]);

    const loadForwarding = useCallback(async (accountId) => {
        if (!accountId) return;
        try {
            const data = await api.getEmailForwarding(accountId);
            setForwardingRules(data.rules || []);
        } catch (err) { toast.error('Failed to load forwarding rules'); }
    }, []);

    useEffect(() => {
        if (activeTab === 'forwarding') {
            if (domains.length === 0) loadDomains();
            // Load all accounts from all domains
            const loadAll = async () => {
                try {
                    const d = await api.getEmailDomains();
                    const all = [];
                    for (const dom of (d.domains || [])) {
                        const accts = await api.getEmailAccounts(dom.id);
                        all.push(...(accts.accounts || []).map(a => ({ ...a, domain_name: dom.name })));
                    }
                    setAllAccounts(all);
                } catch (err) { console.error(err); }
            };
            loadAll();
        }
    }, [activeTab]);

    useEffect(() => { if (selectedAccountId) loadForwarding(selectedAccountId); }, [selectedAccountId]);

    useEffect(() => {
        if (activeTab === 'dns-providers') {
            api.getEmailDNSProviders().then(d => setProviders(d.providers || [])).catch(() => {});
        }
    }, [activeTab]);

    useEffect(() => {
        if (activeTab === 'spam') {
            api.getSpamConfig().then(d => setSpamConfig(d.config || null)).catch(() => {});
        }
    }, [activeTab]);

    useEffect(() => {
        if (activeTab === 'webmail') {
            api.getWebmailStatus().then(d => setWebmailStatus(d)).catch(() => {});
        }
    }, [activeTab]);

    useEffect(() => {
        if (activeTab === 'queue') {
            api.getMailQueue().then(d => setQueue(d.queue || [])).catch(() => {});
            api.getMailLogs(logLines).then(d => setLogs(d.logs || [])).catch(() => {});
        }
    }, [activeTab, logLines]);

    // ── Actions ──

    const handleInstall = async () => {
        setActionLoading(true);
        try {
            await api.installEmailServer({ hostname: installHostname || undefined });
            toast.success('Email server installed');
            loadStatus();
        } catch (err) { toast.error(err.message || 'Installation failed'); }
        finally { setActionLoading(false); }
    };

    const handleServiceControl = async (component, action) => {
        setActionLoading(true);
        try {
            await api.controlEmailService(component, action);
            toast.success(`${component} ${action} successful`);
            loadStatus();
        } catch (err) { toast.error(err.message || `Failed to ${action} ${component}`); }
        finally { setActionLoading(false); }
    };

    const handleAddDomain = async (e) => {
        e.preventDefault();
        setActionLoading(true);
        try {
            await api.addEmailDomain(newDomain);
            toast.success('Domain added');
            setShowDomainForm(false);
            setNewDomain({ name: '', dns_provider_id: '', dns_zone_id: '' });
            loadDomains();
        } catch (err) { toast.error(err.message || 'Failed to add domain'); }
        finally { setActionLoading(false); }
    };

    const handleDeleteDomain = (domainId, name) => {
        setConfirmDialog({
            message: `Delete domain "${name}" and all its accounts and aliases?`,
            onConfirm: async () => {
                try {
                    await api.deleteEmailDomain(domainId);
                    toast.success('Domain deleted');
                    loadDomains();
                } catch (err) { toast.error('Failed to delete domain'); }
                setConfirmDialog(null);
            },
            onCancel: () => setConfirmDialog(null),
        });
    };

    const handleVerifyDNS = async (domainId) => {
        setActionLoading(true);
        try {
            const result = await api.verifyEmailDNS(domainId);
            if (result.all_verified) toast.success('All DNS records verified');
            else toast.error('Some DNS records are missing');
            loadDomains();
        } catch (err) { toast.error('DNS verification failed'); }
        finally { setActionLoading(false); }
    };

    const handleDeployDNS = async (domainId) => {
        setActionLoading(true);
        try {
            await api.deployEmailDNS(domainId);
            toast.success('DNS records deployed');
        } catch (err) { toast.error(err.message || 'DNS deployment failed'); }
        finally { setActionLoading(false); }
    };

    const handleCreateAccount = async (e) => {
        e.preventDefault();
        setActionLoading(true);
        try {
            await api.createEmailAccount(selectedDomainId, newAccount);
            toast.success('Account created');
            setShowAccountForm(false);
            setNewAccount({ username: '', password: '', quota_mb: 1024 });
            loadAccounts(selectedDomainId);
        } catch (err) { toast.error(err.message || 'Failed to create account'); }
        finally { setActionLoading(false); }
    };

    const handleDeleteAccount = (accountId, email) => {
        setConfirmDialog({
            message: `Delete account "${email}"? This will remove the mailbox.`,
            onConfirm: async () => {
                try {
                    await api.deleteEmailAccount(accountId);
                    toast.success('Account deleted');
                    loadAccounts(selectedDomainId);
                } catch (err) { toast.error('Failed to delete account'); }
                setConfirmDialog(null);
            },
            onCancel: () => setConfirmDialog(null),
        });
    };

    const handleChangePassword = async () => {
        if (!showPasswordModal || !newPassword) return;
        setActionLoading(true);
        try {
            await api.changeEmailPassword(showPasswordModal, newPassword);
            toast.success('Password changed');
            setShowPasswordModal(null);
            setNewPassword('');
        } catch (err) { toast.error('Failed to change password'); }
        finally { setActionLoading(false); }
    };

    const handleCreateAlias = async (e) => {
        e.preventDefault();
        setActionLoading(true);
        try {
            await api.createEmailAlias(aliasDomainId, newAlias);
            toast.success('Alias created');
            setShowAliasForm(false);
            setNewAlias({ source: '', destination: '' });
            loadAliases(aliasDomainId);
        } catch (err) { toast.error(err.message || 'Failed to create alias'); }
        finally { setActionLoading(false); }
    };

    const handleDeleteAlias = (aliasId) => {
        setConfirmDialog({
            message: 'Delete this alias?',
            onConfirm: async () => {
                try {
                    await api.deleteEmailAlias(aliasId);
                    toast.success('Alias deleted');
                    loadAliases(aliasDomainId);
                } catch (err) { toast.error('Failed to delete alias'); }
                setConfirmDialog(null);
            },
            onCancel: () => setConfirmDialog(null),
        });
    };

    const handleCreateForwarding = async (e) => {
        e.preventDefault();
        setActionLoading(true);
        try {
            await api.createEmailForwarding(selectedAccountId, newForward);
            toast.success('Forwarding rule created');
            setShowForwardForm(false);
            setNewForward({ destination: '', keep_copy: true });
            loadForwarding(selectedAccountId);
        } catch (err) { toast.error(err.message || 'Failed to create forwarding rule'); }
        finally { setActionLoading(false); }
    };

    const handleDeleteForwarding = (ruleId) => {
        setConfirmDialog({
            message: 'Delete this forwarding rule?',
            onConfirm: async () => {
                try {
                    await api.deleteEmailForwarding(ruleId);
                    toast.success('Rule deleted');
                    loadForwarding(selectedAccountId);
                } catch (err) { toast.error('Failed to delete rule'); }
                setConfirmDialog(null);
            },
            onCancel: () => setConfirmDialog(null),
        });
    };

    const handleAddProvider = async (e) => {
        e.preventDefault();
        setActionLoading(true);
        try {
            await api.addEmailDNSProvider(newProvider);
            toast.success('DNS provider added');
            setShowProviderForm(false);
            setNewProvider({ name: '', provider: 'cloudflare', api_key: '', api_secret: '', api_email: '', is_default: false });
            const d = await api.getEmailDNSProviders();
            setProviders(d.providers || []);
        } catch (err) { toast.error(err.message || 'Failed to add provider'); }
        finally { setActionLoading(false); }
    };

    const handleDeleteProvider = (providerId) => {
        setConfirmDialog({
            message: 'Delete this DNS provider?',
            onConfirm: async () => {
                try {
                    await api.deleteEmailDNSProvider(providerId);
                    toast.success('Provider deleted');
                    const d = await api.getEmailDNSProviders();
                    setProviders(d.providers || []);
                } catch (err) { toast.error('Failed to delete provider'); }
                setConfirmDialog(null);
            },
            onCancel: () => setConfirmDialog(null),
        });
    };

    const handleTestProvider = async (providerId) => {
        setActionLoading(true);
        try {
            const result = await api.testEmailDNSProvider(providerId);
            if (result.success) toast.success('Connection successful');
            else toast.error(result.error || 'Connection failed');
        } catch (err) { toast.error('Test failed'); }
        finally { setActionLoading(false); }
    };

    const handleListZones = async (providerId) => {
        try {
            const result = await api.getEmailDNSZones(providerId);
            setProviderZones(prev => ({ ...prev, [providerId]: result.zones || [] }));
        } catch (err) { toast.error('Failed to list zones'); }
    };

    const handleUpdateSpam = async () => {
        setActionLoading(true);
        try {
            await api.updateSpamConfig(spamConfig);
            toast.success('SpamAssassin config updated');
        } catch (err) { toast.error('Failed to update config'); }
        finally { setActionLoading(false); }
    };

    const handleUpdateSpamRules = async () => {
        setActionLoading(true);
        try {
            const result = await api.updateSpamRules();
            toast.success(result.message || 'Rules updated');
        } catch (err) { toast.error('Failed to update rules'); }
        finally { setActionLoading(false); }
    };

    const handleWebmailInstall = async () => {
        setActionLoading(true);
        try {
            await api.installWebmail({});
            toast.success('Roundcube installed');
            const d = await api.getWebmailStatus();
            setWebmailStatus(d);
        } catch (err) { toast.error('Installation failed'); }
        finally { setActionLoading(false); }
    };

    const handleWebmailControl = async (action) => {
        setActionLoading(true);
        try {
            await api.controlWebmail(action);
            toast.success(`Roundcube ${action} successful`);
            const d = await api.getWebmailStatus();
            setWebmailStatus(d);
        } catch (err) { toast.error(`Failed to ${action}`); }
        finally { setActionLoading(false); }
    };

    const handleConfigureProxy = async () => {
        if (!proxyDomain) return;
        setActionLoading(true);
        try {
            await api.configureWebmailProxy(proxyDomain);
            toast.success('Proxy configured');
        } catch (err) { toast.error('Failed to configure proxy'); }
        finally { setActionLoading(false); }
    };

    const handleFlushQueue = async () => {
        setActionLoading(true);
        try {
            await api.flushMailQueue();
            toast.success('Queue flushed');
            const d = await api.getMailQueue();
            setQueue(d.queue || []);
        } catch (err) { toast.error('Failed to flush queue'); }
        finally { setActionLoading(false); }
    };

    const handleDeleteQueueItem = async (queueId) => {
        try {
            await api.deleteMailQueueItem(queueId);
            toast.success('Message deleted');
            const d = await api.getMailQueue();
            setQueue(d.queue || []);
        } catch (err) { toast.error('Failed to delete message'); }
    };

    // ── Render ──

    if (loading) return <div className="page-container email-page"><EmptyState loading title="Loading email settings" /></div>;

    const isInstalled = status?.installed;

    // Real counts from the loaded domains payload (KPI strip, Domains tab)
    const totalMailboxes = domains.reduce((n, d) => n + (d.accounts_count || 0), 0);
    const totalAliases = domains.reduce((n, d) => n + (d.aliases_count || 0), 0);
    const dnsReadyCount = domains.filter(d => d.dkim_public_key && d.spf_record && d.dmarc_record).length;

    const ServiceCard = ({ name, data, component }) => {
        const Icon = SERVICE_ICONS[component] || Server;
        return (
            <div className="email-service-card">
                <div className="email-service-header">
                    <span className="email-service-ico"><Icon size={15} /></span>
                    <div className="email-service-id">
                        <h3>{name}</h3>
                        {data?.version && <span className="version">v{data.version}</span>}
                    </div>
                    {statusPill(data)}
                </div>
                {data?.installed && (
                    <div className="email-service-actions">
                        <Button size="sm" variant="outline" onClick={() => handleServiceControl(component, 'restart')} disabled={actionLoading}>Restart</Button>
                        {data?.running
                            ? <Button size="sm" variant="outline" onClick={() => handleServiceControl(component, 'stop')} disabled={actionLoading}>Stop</Button>
                            : <Button size="sm" onClick={() => handleServiceControl(component, 'start')} disabled={actionLoading}>Start</Button>
                        }
                    </div>
                )}
            </div>
        );
    };

    return (
        <div className="page-container email-page">
            <PageTopbar
                icon={<Mail size={18} />}
                title="Email Server"
                meta={<>Postfix · Dovecot · DKIM · SpamAssassin · Roundcube</>}
                actions={<Button size="sm" variant="outline" onClick={loadStatus}>Refresh</Button>}
            />

            {!isInstalled ? (
                <div className="not-installed">
                    <div className="not-installed__icon"><Mail size={22} /></div>
                    <h2>Email Server Not Installed</h2>
                    <p>Install Postfix, Dovecot, OpenDKIM, and SpamAssassin to enable email hosting.</p>
                    <div className="install-form">
                        <div className="form-group w-full">
                            <label>Hostname (e.g. mail.example.com)</label>
                            <Input type="text" value={installHostname} onChange={e => setInstallHostname(e.target.value)} placeholder="mail.example.com" />
                        </div>
                        <Button onClick={handleInstall} disabled={actionLoading}>
                            {actionLoading ? 'Installing...' : 'Install Email Server'}
                        </Button>
                    </div>
                </div>
            ) : (
                <>
                    <Tabs value={activeTab} onValueChange={setActiveTab}>
                        <TabsList>
                            {VALID_TABS.map(tab => (
                                <TabsTrigger key={tab} value={tab}>
                                    {TAB_LABELS[tab] || tab.charAt(0).toUpperCase() + tab.slice(1)}
                                </TabsTrigger>
                            ))}
                        </TabsList>

                        {/* Status Tab */}
                        <TabsContent value="status">
                            <div className="email-status">
                                <div className="status-grid">
                                    <ServiceCard name="Postfix (SMTP)" data={status?.postfix} component="postfix" />
                                    <ServiceCard name="Dovecot (IMAP)" data={status?.dovecot} component="dovecot" />
                                    <ServiceCard name="OpenDKIM" data={status?.dkim} component="opendkim" />
                                    <ServiceCard name="SpamAssassin" data={status?.spamassassin} component="spamassassin" />
                                    <ServiceCard name="Roundcube" data={status?.roundcube} component="roundcube" />
                                </div>
                            </div>
                        </TabsContent>

                        {/* Domains Tab */}
                        <TabsContent value="domains">
                            <div className="email-domains">
                                {domains.length > 0 && (
                                    <div className="email-kpis">
                                        <MetricCard icon={<Globe size={17} />} tone="accent" value={domains.length} label="Mail domains" />
                                        <MetricCard icon={<AtSign size={17} />} tone="cyan" value={totalMailboxes} label="Mailboxes" />
                                        <MetricCard icon={<Forward size={17} />} tone="violet" value={totalAliases} label="Aliases" />
                                        <MetricCard icon={<ShieldCheck size={17} />} tone="green" value={dnsReadyCount} unit={`/ ${domains.length}`} label="DNS configured" />
                                    </div>
                                )}
                                <div className="section-header">
                                    <h2>Email Domains</h2>
                                    <Button size="sm" variant={showDomainForm ? 'outline' : 'default'} onClick={() => setShowDomainForm(!showDomainForm)}>
                                        {showDomainForm ? 'Cancel' : 'Add Domain'}
                                    </Button>
                                </div>
                                {showDomainForm && (
                                    <form className="email-form" onSubmit={handleAddDomain}>
                                        <div className="form-grid">
                                            <div className="form-group">
                                                <label>Domain Name</label>
                                                <Input type="text" value={newDomain.name} onChange={e => setNewDomain({ ...newDomain, name: e.target.value })} placeholder="example.com" required />
                                            </div>
                                        </div>
                                        <div className="form-actions">
                                            <Button type="submit" size="sm" disabled={actionLoading}>Add Domain</Button>
                                        </div>
                                    </form>
                                )}
                                {domains.length === 0 ? (
                                    <EmptyState icon={Globe} title="No domains configured" />
                                ) : (
                                    <div className="email-table-card">
                                        <DataTable
                                            tableClassName="sk-dtable"
                                            sortable={false}
                                            data={domains}
                                            keyField="id"
                                            columns={[
                                                {
                                                    key: 'domain',
                                                    header: 'Domain',
                                                    render: (d) => (
                                                        <div className="sk-cell-name">
                                                            <span className="email-fav"><Globe size={15} /></span>
                                                            {d.name}
                                                        </div>
                                                    ),
                                                },
                                                { key: 'accounts', header: 'Mailboxes', render: (d) => <span className="sk-cell-mono">{d.accounts_count}</span> },
                                                { key: 'aliases', header: 'Aliases', render: (d) => <span className="sk-cell-mono">{d.aliases_count}</span> },
                                                { key: 'dkim', header: 'DKIM', render: (d) => dnsPill(d.dkim_public_key) },
                                                { key: 'spf', header: 'SPF', render: (d) => dnsPill(d.spf_record) },
                                                { key: 'dmarc', header: 'DMARC', render: (d) => dnsPill(d.dmarc_record) },
                                                {
                                                    key: 'status',
                                                    header: 'Status',
                                                    render: (d) => <Pill kind={d.is_active ? 'green' : 'gray'}>{d.is_active ? 'active' : 'inactive'}</Pill>,
                                                },
                                                {
                                                    key: 'actions',
                                                    header: '',
                                                    render: (d) => (
                                                        <div className="email-row-actions">
                                                            <Button size="sm" variant="outline" onClick={() => handleVerifyDNS(d.id)} disabled={actionLoading}>Verify DNS</Button>
                                                            {d.dns_provider_id && <Button size="sm" onClick={() => handleDeployDNS(d.id)} disabled={actionLoading}>Deploy DNS</Button>}
                                                            <Button size="sm" variant="destructive" onClick={() => handleDeleteDomain(d.id, d.name)}>Delete</Button>
                                                        </div>
                                                    ),
                                                },
                                            ]}
                                        />
                                    </div>
                                )}
                            </div>
                        </TabsContent>

                        {/* Accounts Tab */}
                        <TabsContent value="accounts">
                            <div className="email-accounts">
                                <div className="domain-selector">
                                    <div className="form-group">
                                        <label>Select Domain</label>
                                        <select value={selectedDomainId} onChange={e => setSelectedDomainId(e.target.value)}>
                                            <option value="">-- Select --</option>
                                            {domains.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
                                        </select>
                                    </div>
                                </div>
                                {selectedDomainId && (
                                    <>
                                        <div className="section-header">
                                            <h2>Accounts</h2>
                                            <Button size="sm" variant={showAccountForm ? 'outline' : 'default'} onClick={() => setShowAccountForm(!showAccountForm)}>
                                                {showAccountForm ? 'Cancel' : 'Create Account'}
                                            </Button>
                                        </div>
                                        {showAccountForm && (
                                            <form className="email-form" onSubmit={handleCreateAccount}>
                                                <div className="form-grid">
                                                    <div className="form-group">
                                                        <label>Username</label>
                                                        <Input type="text" value={newAccount.username} onChange={e => setNewAccount({ ...newAccount, username: e.target.value })} placeholder="user" required />
                                                    </div>
                                                    <div className="form-group">
                                                        <label>Password</label>
                                                        <Input type="password" value={newAccount.password} onChange={e => setNewAccount({ ...newAccount, password: e.target.value })} required />
                                                    </div>
                                                    <div className="form-group">
                                                        <label>Quota (MB)</label>
                                                        <Input type="number" value={newAccount.quota_mb} onChange={e => setNewAccount({ ...newAccount, quota_mb: parseInt(e.target.value) || 1024 })} />
                                                    </div>
                                                </div>
                                                <div className="form-actions">
                                                    <Button type="submit" size="sm" disabled={actionLoading}>Create</Button>
                                                </div>
                                            </form>
                                        )}
                                        {accounts.length === 0 ? (
                                            <EmptyState icon={AtSign} title="No accounts for this domain" />
                                        ) : (
                                            <div className="email-table-card">
                                                <table className="sk-dtable">
                                                    <thead>
                                                        <tr>
                                                            <th>Address</th>
                                                            <th>Quota</th>
                                                            <th>Status</th>
                                                            <th />
                                                        </tr>
                                                    </thead>
                                                    <tbody>
                                                        {accounts.map(a => (
                                                            <tr key={a.id}>
                                                                <td>
                                                                    <div className="sk-cell-name">
                                                                        <span className="email-fav email-fav--cyan"><AtSign size={14} /></span>
                                                                        <span className="email-addr">{a.email}</span>
                                                                    </div>
                                                                </td>
                                                                <td className="sk-cell-mono">{a.quota_mb} MB</td>
                                                                <td><Pill kind={a.is_active ? 'green' : 'gray'}>{a.is_active ? 'active' : 'disabled'}</Pill></td>
                                                                <td>
                                                                    <div className="email-row-actions">
                                                                        <Button size="sm" variant="outline" onClick={() => { setShowPasswordModal(a.id); setNewPassword(''); }}>Password</Button>
                                                                        <Button size="sm" variant="destructive" onClick={() => handleDeleteAccount(a.id, a.email)}>Delete</Button>
                                                                    </div>
                                                                </td>
                                                            </tr>
                                                        ))}
                                                    </tbody>
                                                </table>
                                            </div>
                                        )}
                                    </>
                                )}
                                <Modal open={!!showPasswordModal} onClose={() => setShowPasswordModal(null)} title="Change Password">
                                    <div className="form-group">
                                        <label>New Password</label>
                                        <Input type="password" value={newPassword} onChange={e => setNewPassword(e.target.value)} />
                                    </div>
                                    <div className="form-actions">
                                        <Button size="sm" variant="outline" onClick={() => setShowPasswordModal(null)}>Cancel</Button>
                                        <Button size="sm" onClick={handleChangePassword} disabled={actionLoading || !newPassword}>Change</Button>
                                    </div>
                                </Modal>
                            </div>
                        </TabsContent>

                        {/* Aliases Tab */}
                        <TabsContent value="aliases">
                            <div className="email-aliases">
                                <div className="domain-selector">
                                    <div className="form-group">
                                        <label>Select Domain</label>
                                        <select value={aliasDomainId} onChange={e => setAliasDomainId(e.target.value)}>
                                            <option value="">-- Select --</option>
                                            {domains.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
                                        </select>
                                    </div>
                                </div>
                                {aliasDomainId && (
                                    <>
                                        <div className="section-header">
                                            <h2>Aliases</h2>
                                            <Button size="sm" variant={showAliasForm ? 'outline' : 'default'} onClick={() => setShowAliasForm(!showAliasForm)}>
                                                {showAliasForm ? 'Cancel' : 'Create Alias'}
                                            </Button>
                                        </div>
                                        {showAliasForm && (
                                            <form className="email-form" onSubmit={handleCreateAlias}>
                                                <div className="form-grid">
                                                    <div className="form-group">
                                                        <label>Source</label>
                                                        <Input type="text" value={newAlias.source} onChange={e => setNewAlias({ ...newAlias, source: e.target.value })} placeholder="info@example.com" required />
                                                    </div>
                                                    <div className="form-group">
                                                        <label>Destination</label>
                                                        <Input type="text" value={newAlias.destination} onChange={e => setNewAlias({ ...newAlias, destination: e.target.value })} placeholder="user@example.com" required />
                                                    </div>
                                                </div>
                                                <div className="form-actions">
                                                    <Button type="submit" size="sm" disabled={actionLoading}>Create</Button>
                                                </div>
                                            </form>
                                        )}
                                        {aliases.length === 0 ? (
                                            <EmptyState icon={Forward} title="No aliases for this domain" />
                                        ) : (
                                            <div className="email-table-card">
                                                <table className="sk-dtable">
                                                    <thead>
                                                        <tr>
                                                            <th>Source</th>
                                                            <th style={{ width: 30 }} />
                                                            <th>Destination</th>
                                                            <th />
                                                        </tr>
                                                    </thead>
                                                    <tbody>
                                                        {aliases.map(a => (
                                                            <tr key={a.id}>
                                                                <td className="sk-cell-mono">{a.source}</td>
                                                                <td className="email-arrow">&rarr;</td>
                                                                <td className="sk-cell-mono">{a.destination}</td>
                                                                <td>
                                                                    <div className="email-row-actions">
                                                                        <Button size="sm" variant="destructive" onClick={() => handleDeleteAlias(a.id)}>Delete</Button>
                                                                    </div>
                                                                </td>
                                                            </tr>
                                                        ))}
                                                    </tbody>
                                                </table>
                                            </div>
                                        )}
                                    </>
                                )}
                            </div>
                        </TabsContent>

                        {/* Forwarding Tab */}
                        <TabsContent value="forwarding">
                            <div className="email-forwarding">
                                <div className="domain-selector">
                                    <div className="form-group">
                                        <label>Select Account</label>
                                        <select value={selectedAccountId} onChange={e => setSelectedAccountId(e.target.value)}>
                                            <option value="">-- Select --</option>
                                            {allAccounts.map(a => <option key={a.id} value={a.id}>{a.email}</option>)}
                                        </select>
                                    </div>
                                </div>
                                {selectedAccountId && (
                                    <>
                                        <div className="section-header">
                                            <h2>Forwarding Rules</h2>
                                            <Button size="sm" variant={showForwardForm ? 'outline' : 'default'} onClick={() => setShowForwardForm(!showForwardForm)}>
                                                {showForwardForm ? 'Cancel' : 'Add Rule'}
                                            </Button>
                                        </div>
                                        {showForwardForm && (
                                            <form className="email-form" onSubmit={handleCreateForwarding}>
                                                <div className="form-grid">
                                                    <div className="form-group">
                                                        <label>Forward To</label>
                                                        <Input type="email" value={newForward.destination} onChange={e => setNewForward({ ...newForward, destination: e.target.value })} required />
                                                    </div>
                                                    <div className="form-group">
                                                        <label className="email-check">
                                                            <input type="checkbox" checked={newForward.keep_copy} onChange={e => setNewForward({ ...newForward, keep_copy: e.target.checked })} />
                                                            {' '}Keep a copy in mailbox
                                                        </label>
                                                    </div>
                                                </div>
                                                <div className="form-actions">
                                                    <Button type="submit" size="sm" disabled={actionLoading}>Add</Button>
                                                </div>
                                            </form>
                                        )}
                                        {forwardingRules.length === 0 ? (
                                            <EmptyState icon={Forward} title="No forwarding rules" />
                                        ) : (
                                            <div className="email-table-card">
                                                <table className="sk-dtable">
                                                    <thead>
                                                        <tr>
                                                            <th>Mailbox</th>
                                                            <th style={{ width: 30 }} />
                                                            <th>Forward To</th>
                                                            <th>Copy</th>
                                                            <th>Status</th>
                                                            <th />
                                                        </tr>
                                                    </thead>
                                                    <tbody>
                                                        {forwardingRules.map(r => (
                                                            <tr key={r.id}>
                                                                <td className="sk-cell-mono">{r.account_email}</td>
                                                                <td className="email-arrow">&rarr;</td>
                                                                <td className="sk-cell-mono">{r.destination}</td>
                                                                <td><Pill kind={r.keep_copy ? 'cyan' : 'gray'} dot={false}>{r.keep_copy ? 'keeps copy' : 'no copy'}</Pill></td>
                                                                <td><Pill kind={r.is_active ? 'green' : 'gray'}>{r.is_active ? 'active' : 'inactive'}</Pill></td>
                                                                <td>
                                                                    <div className="email-row-actions">
                                                                        <Button size="sm" variant="destructive" onClick={() => handleDeleteForwarding(r.id)}>Delete</Button>
                                                                    </div>
                                                                </td>
                                                            </tr>
                                                        ))}
                                                    </tbody>
                                                </table>
                                            </div>
                                        )}
                                    </>
                                )}
                            </div>
                        </TabsContent>

                        {/* DNS Providers Tab */}
                        <TabsContent value="dns-providers">
                            <div className="email-dns-providers">
                                <div className="section-header">
                                    <h2>DNS Providers</h2>
                                    <Button size="sm" variant={showProviderForm ? 'outline' : 'default'} onClick={() => setShowProviderForm(!showProviderForm)}>
                                        {showProviderForm ? 'Cancel' : 'Add Provider'}
                                    </Button>
                                </div>
                                {showProviderForm && (
                                    <form className="email-form" onSubmit={handleAddProvider}>
                                        <div className="form-grid">
                                            <div className="form-group"><label>Name</label><Input type="text" value={newProvider.name} onChange={e => setNewProvider({ ...newProvider, name: e.target.value })} required /></div>
                                            <div className="form-group">
                                                <label>Provider</label>
                                                <select value={newProvider.provider} onChange={e => setNewProvider({ ...newProvider, provider: e.target.value })}>
                                                    <option value="cloudflare">Cloudflare</option>
                                                    <option value="route53">Route53</option>
                                                </select>
                                            </div>
                                            <div className="form-group"><label>API Key</label><Input type="password" value={newProvider.api_key} onChange={e => setNewProvider({ ...newProvider, api_key: e.target.value })} required /></div>
                                            <div className="form-group"><label>API Secret (Route53)</label><Input type="password" value={newProvider.api_secret} onChange={e => setNewProvider({ ...newProvider, api_secret: e.target.value })} /></div>
                                            <div className="form-group"><label>API Email (Cloudflare)</label><Input type="email" value={newProvider.api_email} onChange={e => setNewProvider({ ...newProvider, api_email: e.target.value })} /></div>
                                        </div>
                                        <div className="form-actions"><Button type="submit" size="sm" disabled={actionLoading}>Add</Button></div>
                                    </form>
                                )}
                                <div className="provider-list">
                                    {providers.length === 0 ? (
                                        <EmptyState icon={Server} title="No DNS providers configured" />
                                    ) : providers.map(p => (
                                        <div key={p.id} className="provider-card">
                                            <div className="provider-header">
                                                <span className="email-fav email-fav--violet"><Server size={15} /></span>
                                                <h3>{p.name}</h3>
                                                <span className="provider-type">{p.provider}</span>
                                                {p.is_default && <Pill kind="cyan" dot={false}>default</Pill>}
                                            </div>
                                            <div className="provider-meta">
                                                <div className="meta-row"><span className="k">API key</span><span className="v">{p.api_key}</span></div>
                                            </div>
                                            <div className="provider-actions">
                                                <Button size="sm" variant="outline" onClick={() => handleTestProvider(p.id)} disabled={actionLoading}>Test</Button>
                                                <Button size="sm" variant="outline" onClick={() => handleListZones(p.id)}>Zones</Button>
                                                <Button size="sm" variant="destructive" onClick={() => handleDeleteProvider(p.id)}>Delete</Button>
                                            </div>
                                            {providerZones[p.id] && (
                                                <div className="zones-list">
                                                    {providerZones[p.id].map(z => (
                                                        <div key={z.id} className="zone-item"><span>{z.name}</span><span>{z.id}</span></div>
                                                    ))}
                                                </div>
                                            )}
                                        </div>
                                    ))}
                                </div>
                            </div>
                        </TabsContent>

                        {/* Spam Tab */}
                        <TabsContent value="spam">
                            {spamConfig && (
                                <div className="email-spam">
                                    <div className="section-header">
                                        <h2>SpamAssassin Configuration</h2>
                                        <div className="section-actions">
                                            <Button size="sm" variant="outline" onClick={handleUpdateSpamRules} disabled={actionLoading}>Update Rules</Button>
                                            <Button size="sm" onClick={handleUpdateSpam} disabled={actionLoading}>Save</Button>
                                        </div>
                                    </div>
                                    <div className="spam-config">
                                        <div className="form-grid">
                                            <div className="form-group">
                                                <label>Required Score</label>
                                                <Input type="number" step="0.1" value={spamConfig.required_score} onChange={e => setSpamConfig({ ...spamConfig, required_score: parseFloat(e.target.value) })} />
                                            </div>
                                            <div className="form-group">
                                                <label>Rewrite Subject</label>
                                                <Input type="text" value={spamConfig.rewrite_subject} onChange={e => setSpamConfig({ ...spamConfig, rewrite_subject: e.target.value })} />
                                            </div>
                                            <div className="form-group checkbox-field">
                                                <input type="checkbox" checked={!!spamConfig.use_bayes} onChange={e => setSpamConfig({ ...spamConfig, use_bayes: e.target.checked ? 1 : 0 })} />
                                                <label>Enable Bayesian Filter</label>
                                            </div>
                                            <div className="form-group checkbox-field">
                                                <input type="checkbox" checked={!!spamConfig.bayes_auto_learn} onChange={e => setSpamConfig({ ...spamConfig, bayes_auto_learn: e.target.checked ? 1 : 0 })} />
                                                <label>Auto-learn</label>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            )}
                        </TabsContent>

                        {/* Webmail Tab */}
                        <TabsContent value="webmail">
                            <div className="email-webmail">
                                <div className="section-header"><h2>Roundcube Webmail</h2></div>
                                <div className="webmail-card">
                                    <div className="webmail-status-row">
                                        {statusPill(webmailStatus)}
                                        {webmailStatus?.port && <span className="webmail-port">Port: {webmailStatus.port}</span>}
                                    </div>
                                    <div className="webmail-actions">
                                        {!webmailStatus?.installed ? (
                                            <Button size="sm" onClick={handleWebmailInstall} disabled={actionLoading}>Install Roundcube</Button>
                                        ) : (
                                            <>
                                                {webmailStatus?.running
                                                    ? <Button size="sm" variant="outline" onClick={() => handleWebmailControl('stop')} disabled={actionLoading}>Stop</Button>
                                                    : <Button size="sm" onClick={() => handleWebmailControl('start')} disabled={actionLoading}>Start</Button>
                                                }
                                                <Button size="sm" variant="outline" onClick={() => handleWebmailControl('restart')} disabled={actionLoading}>Restart</Button>
                                            </>
                                        )}
                                    </div>
                                    {webmailStatus?.installed && (
                                        <div className="proxy-form">
                                            <div className="form-group">
                                                <label>Proxy Domain</label>
                                                <Input type="text" value={proxyDomain} onChange={e => setProxyDomain(e.target.value)} placeholder="webmail.example.com" />
                                            </div>
                                            <Button size="sm" onClick={handleConfigureProxy} disabled={actionLoading || !proxyDomain}>Configure Nginx Proxy</Button>
                                        </div>
                                    )}
                                </div>
                            </div>
                        </TabsContent>

                        {/* Queue & Logs Tab */}
                        <TabsContent value="queue">
                            <div>
                                <div className="email-queue">
                                    <div className="section-header">
                                        <h2>Mail Queue ({queue.length})</h2>
                                        <Button size="sm" variant="outline" onClick={handleFlushQueue} disabled={actionLoading}>Flush Queue</Button>
                                    </div>
                                    {queue.length === 0 ? (
                                        <EmptyState icon={Inbox} title="Queue is empty" />
                                    ) : (
                                        <div className="email-table-card">
                                            <table className="sk-dtable">
                                                <thead>
                                                    <tr>
                                                        <th>Queue ID</th>
                                                        <th>From</th>
                                                        <th>Recipients</th>
                                                        <th>Size</th>
                                                        <th>Arrived</th>
                                                        <th />
                                                    </tr>
                                                </thead>
                                                <tbody>
                                                    {queue.map(item => (
                                                        <tr key={item.queue_id}>
                                                            <td className="sk-cell-mono email-qid">{item.queue_id}</td>
                                                            <td className="sk-cell-mono">{item.sender}</td>
                                                            <td>
                                                                <span className="sk-cell-mono">{(item.recipients || []).join(', ') || '—'}</span>
                                                                {item.error && <div className="email-qerr">{item.error}</div>}
                                                            </td>
                                                            <td className="sk-cell-mono">{item.size}B</td>
                                                            <td className="sk-cell-mono">{item.arrival_time}</td>
                                                            <td>
                                                                <div className="email-row-actions">
                                                                    <Button size="sm" variant="destructive" onClick={() => handleDeleteQueueItem(item.queue_id)}>Delete</Button>
                                                                </div>
                                                            </td>
                                                        </tr>
                                                    ))}
                                                </tbody>
                                            </table>
                                        </div>
                                    )}
                                </div>
                                <div className="email-logs">
                                    <div className="section-header">
                                        <h2>Mail Logs</h2>
                                        <div className="log-controls">
                                            <select value={logLines} onChange={e => setLogLines(parseInt(e.target.value))}>
                                                <option value={50}>50 lines</option>
                                                <option value={100}>100 lines</option>
                                                <option value={500}>500 lines</option>
                                            </select>
                                        </div>
                                    </div>
                                    <pre className="log-output">{logs.length > 0 ? logs.join('\n') : 'No logs available'}</pre>
                                </div>
                            </div>
                        </TabsContent>
                    </Tabs>
                </>
            )}

            {confirmDialog && <ConfirmDialog {...confirmDialog} />}
        </div>
    );
}

export default Email;
