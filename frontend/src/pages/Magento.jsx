// Magento stack management — sk-magento fork extension.
// Data-plane stack creation is separate from optional Composer/setup:install,
// with custom Magento and frontend source paths supported.
import { useState, useEffect, useCallback, useRef } from 'react';
import api from '../services/api';
import { useToast } from '../contexts/ToastContext';
import { useConfirm } from '../hooks/useConfirm';
import EmptyState from '../components/EmptyState';
import Modal from '@/components/Modal';
import {
    ShoppingBag, Plus, RefreshCw, Trash2, Activity, Globe,
    Zap, ScrollText, Copy, ExternalLink, Database, Search as SearchIcon,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { MetricCard, Pill, PageTopbar } from '@/components/ds';

const STATUS_KIND = {
    running: 'green',
    provisioning: 'cyan',
    failed: 'red',
    stopped: 'gray',
};

// Curated quick actions surfaced as buttons; the rest sit in the picker.
const PRIMARY_ACTIONS = [
    { id: 'cache-flush', label: 'Flush Cache' },
    { id: 'reindex', label: 'Reindex' },
    { id: 'setup-upgrade', label: 'Setup Upgrade' },
    { id: 'cron-run', label: 'Run Cron' },
];

const SERVICE_FIELDS = [
    {
        id: 'db',
        label: 'Database image',
        helper: 'MariaDB/MySQL image used by the stack database service.',
        options: [
            ['mariadb:11.8', 'MariaDB 11.8 — Magento 2.4.8-p5 latest'],
            ['mariadb:11.4', 'MariaDB 11.4 — Magento 2.4.8'],
            ['mysql:8.4', 'MySQL 8.4 — Magento 2.4.8'],
            ['mariadb:10.11', 'MariaDB 10.11 — Magento 2.4.7/2.4.6'],
            ['mariadb:10.6', 'MariaDB 10.6 — legacy/default'],
        ],
    },
    {
        id: 'opensearch',
        label: 'Search image',
        helper: 'OpenSearch image. Use custom only if the compose env is compatible.',
        options: [
            ['opensearchproject/opensearch:3', 'OpenSearch 3 — Magento 2.4.8-p5'],
            ['opensearchproject/opensearch:3.1.0', 'OpenSearch 3.1'],
            ['opensearchproject/opensearch:2.19.0', 'OpenSearch 2.19 — Magento 2.4.7/2.4.6'],
            ['opensearchproject/opensearch:2.12.0', 'OpenSearch 2.12 — ServerKit default'],
            ['opensearchproject/opensearch:2.5.0', 'OpenSearch 2.5 — legacy stack'],
        ],
    },
    {
        id: 'redis',
        label: 'Cache image',
        helper: 'Redis-protocol cache/session service. Valkey is preferred for newer Magento patches.',
        options: [
            ['valkey/valkey:8.1-alpine', 'Valkey 8.1 — Magento 2.4.8-p5'],
            ['valkey/valkey:8-alpine', 'Valkey 8'],
            ['redis:7.2-alpine', 'Redis 7.2 — legacy/default'],
        ],
    },
    {
        id: 'rabbitmq',
        label: 'RabbitMQ image',
        helper: 'Used only when RabbitMQ is enabled below.',
        options: [
            ['rabbitmq:4.2-management-alpine', 'RabbitMQ 4.2 — Magento latest patches'],
            ['rabbitmq:4.1-management-alpine', 'RabbitMQ 4.1 — Magento 2.4.8-p4/2.4.7-p9'],
            ['rabbitmq:3.13-management-alpine', 'RabbitMQ 3.13 — ServerKit default'],
            ['rabbitmq:3.9-management-alpine', 'RabbitMQ 3.9 — Magento 2.4.4'],
        ],
    },
    {
        id: 'varnish',
        label: 'Varnish image',
        helper: 'Used only when Varnish FPC is enabled below.',
        options: [
            ['varnish:8', 'Varnish 8 — Magento latest patches'],
            ['varnish:7.7', 'Varnish 7.7'],
            ['varnish:7.4', 'Varnish 7.4 — ServerKit default'],
        ],
    },
    {
        id: 'mailpit',
        label: 'Mail capture image',
        helper: 'Development SMTP/UI mail capture service.',
        options: [
            ['axllent/mailpit:latest', 'Mailpit latest — ServerKit default'],
            ['mailhog/mailhog:latest', 'MailHog latest — legacy stack'],
            ['mailhog/mailhog', 'MailHog unpinned — legacy exact image'],
        ],
    },
];

const DEFAULT_SERVICE_VALUE = '__serverkit_default__';
const CUSTOM_IMAGE_VALUE = '__custom_image__';

const serviceOptionValues = (field) => field.options.map(([value]) => value);

const initialMagentoForm = () => ({
    name: '',
    domain: '',
    api_domain: '',
    magento_version: '2.4.8',
    distribution: 'mage-os',
    php_version: '',
    install_magento: false,
    auto_install_php: true,
    php_extension_profile: 'magento',
    root_path: '',
    magento_source_path: '',
    ssl: 'none',
    use_rabbitmq: false,
    use_varnish: false,
    headless_mode: 'none',
    split_route_mode: 'api_only',
    frontend_domain: '',
    frontend_port: 3000,
    frontend_root: '',
    magento_routes: '',
    admin_domain: '',
    frontend_cmd: '',
    run_user: 'www-data',
    le_challenge: 'dns',
    le_email: '',
    service_versions: {},
});

const Magento = () => {
    const toast = useToast();
    const { confirm } = useConfirm();
    const [stores, setStores] = useState([]);
    const [versions, setVersions] = useState([]);
    const [catalog, setCatalog] = useState({ service_versions: {}, ssl_modes: ['none','self-signed','letsencrypt'] });
    const [actions, setActions] = useState([]);
    const [loading, setLoading] = useState(true);

    const [showCreate, setShowCreate] = useState(false);
    const [creating, setCreating] = useState(false);
    const [form, setForm] = useState(initialMagentoForm());
    const [credentials, setCredentials] = useState(null);

    const [logStore, setLogStore] = useState(null);
    const [logLines, setLogLines] = useState([]);
    const [healthStore, setHealthStore] = useState(null);
    const [health, setHealth] = useState(null);
    const [busyAction, setBusyAction] = useState(null);
    const [actionOutput, setActionOutput] = useState(null);
    const pollRef = useRef(null);

    const load = useCallback(async () => {
        try {
            const [s, v, a] = await Promise.all([
                api.getMagentoStores(),
                api.getMagentoVersions(),
                api.getMagentoActions(),
            ]);
            setStores(s.stores || []);
            setVersions(v.versions || []); setCatalog(v);
            setActions(a.actions || []);
        } catch (err) {
            toast.error(`Failed to load Magento stores: ${err.message}`);
        } finally {
            setLoading(false);
        }
    }, [toast]);

    useEffect(() => { load(); }, [load]);

    const serviceValue = (field) => form.service_versions?.[field.id] || '';

    const serviceSelectValue = (field) => {
        const current = serviceValue(field);
        if (!current) return DEFAULT_SERVICE_VALUE;
        return serviceOptionValues(field).includes(current) ? current : CUSTOM_IMAGE_VALUE;
    };

    const updateServiceVersion = (serviceId, value) => {
        setForm((prev) => {
            const nextVersions = { ...(prev.service_versions || {}) };
            if (value) nextVersions[serviceId] = value;
            else delete nextVersions[serviceId];
            return { ...prev, service_versions: nextVersions };
        });
    };

    const handleServiceChoice = (field, value) => {
        if (value === DEFAULT_SERVICE_VALUE) {
            updateServiceVersion(field.id, '');
            return;
        }
        if (value === CUSTOM_IMAGE_VALUE) {
            const current = serviceValue(field);
            updateServiceVersion(field.id, serviceOptionValues(field).includes(current) ? '' : current);
            return;
        }
        updateServiceVersion(field.id, value);
    };

    // Poll while any store is provisioning
    useEffect(() => {
        const provisioning = stores.some((s) => s.status === 'provisioning');
        if (provisioning && !pollRef.current) {
            pollRef.current = setInterval(async () => {
                const s = await api.getMagentoStores().catch(() => null);
                if (s) setStores(s.stores || []);
            }, 5000);
        }
        if (!provisioning && pollRef.current) {
            clearInterval(pollRef.current);
            pollRef.current = null;
        }
        return () => { if (pollRef.current) { clearInterval(pollRef.current); pollRef.current = null; } };
    }, [stores]);

    const createStore = async () => {
        const primaryDomain = form.headless_mode === 'split' ? form.api_domain : form.domain;
        if (!form.name || !primaryDomain) {
            toast.error(form.headless_mode === 'split' ? 'Name and API domain are required' : 'Name and domain are required');
            return;
        }
        setCreating(true);
        try {
            const payload = { ...form };
            if (payload.headless_mode === 'split') payload.domain = payload.api_domain;
            payload.frontend_port = Number(payload.frontend_port) || 0;
            payload.install_magento = Boolean(form.install_magento);
            payload.magento_routes = (form.magento_routes || '').split(',').map((r) => r.trim()).filter(Boolean);
            if (!payload.api_domain) delete payload.api_domain;
            if (payload.headless_mode !== 'split') delete payload.split_route_mode;
            if (!payload.frontend_domain) delete payload.frontend_domain;
            if (!payload.php_version) delete payload.php_version;
            if (!payload.auto_install_php) delete payload.php_extension_profile;
            if (!payload.root_path) delete payload.root_path;
            if (!payload.magento_source_path) delete payload.magento_source_path;
            if (!payload.frontend_root) delete payload.frontend_root;
            if (!payload.admin_domain) delete payload.admin_domain;
            if (!payload.frontend_cmd) delete payload.frontend_cmd;
            if (form.ssl === 'letsencrypt') { payload.le_challenge = form.le_challenge; if (form.le_email) payload.le_email = form.le_email; }
            payload.run_user = form.run_user || 'www-data';
            const svOverrides = Object.fromEntries(Object.entries(form.service_versions || {}).filter(([, val]) => val && val.trim()));
            if (Object.keys(svOverrides).length) payload.service_versions = svOverrides; else delete payload.service_versions;
            delete payload.le_email; if (form.ssl==='letsencrypt' && form.le_email) payload.le_email = form.le_email;
            const res = await api.createMagentoStore(payload);
            setCredentials(res.store);
            toast.success(payload.install_magento ? 'Magento initialization started' : 'Magento stack creation started');
            setShowCreate(false);
            setForm(initialMagentoForm());
            load();
        } catch (err) {
            toast.error(err.message);
        } finally {
            setCreating(false);
        }
    };

    const removeStore = async (store) => {
        const ok = await confirm({
            title: `Delete store ${store.name}`,
            message: 'This removes the vhost, cron entry, and data-plane containers WITH their volumes. Store files stay on disk unless purged.',
        });
        if (!ok) return;
        try {
            await api.deleteMagentoStore(store.id, false);
            toast.success(`Store ${store.name} removed`);
            load();
        } catch (err) {
            toast.error(err.message);
        }
    };

    const runAction = async (store, actionId) => {
        setBusyAction(`${store.id}:${actionId}`);
        setActionOutput(null);
        try {
            const res = await api.runMagentoAction(store.id, actionId);
            setActionOutput({ store: store.name, action: actionId, ...res });
            if (res.success) toast.success(`${actionId} completed on ${store.name}`);
        } catch (err) {
            toast.error(`${actionId} failed: ${err.message}`);
        } finally {
            setBusyAction(null);
        }
    };

    const retryProvision = async (store) => {
        setBusyAction(`${store.id}:retry-provision`);
        try {
            await api.retryMagentoProvision(store.id);
            toast.success(`Provisioning retry started for ${store.name}`);
            load();
        } catch (err) {
            toast.error(`Retry failed: ${err.message}`);
        } finally {
            setBusyAction(null);
        }
    };

    const openLog = async (store) => {
        setLogStore(store);
        const res = await api.getMagentoStoreLog(store.id, 200).catch(() => ({ lines: [] }));
        setLogLines(res.lines || []);
    };

    const openHealth = async (store) => {
        setHealthStore(store);
        setHealth(null);
        const res = await api.getMagentoStoreHealth(store.id).catch(() => null);
        setHealth(res);
    };

    // ── Web Config (PATCH + apply-web + editable vhost) ──────────────
    const [webStore, setWebStore] = useState(null);
    const [webForm, setWebForm] = useState(null);
    const [webTab, setWebTab] = useState('config');
    const [vhostText, setVhostText] = useState('');
    const [webBusy, setWebBusy] = useState(false);
    const [certDays, setCertDays] = useState(null);

    const openWeb = (store) => {
        setWebStore(store);
        setWebTab('config');
        setWebForm({
            ssl: store.ssl_mode || 'none',
            run_user: store.run_user || 'www-data',
            le_challenge: store.le_challenge || 'dns',
            le_email: store.le_email || '',
            headless_mode: store.headless_mode || 'none',
            api_domain: store.api_domain || '',
            split_route_mode: store.split_route_mode || 'api_only',
            frontend_domain: store.frontend_domain || '',
            admin_domain: store.admin_domain || '',
            frontend_port: store.frontend_port ?? 3000,
            frontend_root: store.frontend_root || '',
            frontend_cmd: store.frontend_cmd || '',
            magento_routes: (store.magento_routes || []).join(', '),
        });
        setCertDays(store.ssl_cert_days);
        setVhostText('');
    };

    const saveAndApply = async () => {
        setWebBusy(true);
        try {
            const payload = {
                ssl: webForm.ssl,
                headless_mode: webForm.headless_mode,
                api_domain: webForm.api_domain,
                split_route_mode: webForm.split_route_mode,
                frontend_port: Number(webForm.frontend_port) || 0,
                magento_routes: (webForm.magento_routes || '').split(',').map((r) => r.trim()).filter(Boolean),
                run_user: webForm.run_user || 'www-data',
            };
            if (webForm.headless_mode !== 'split') {
                delete payload.api_domain;
                delete payload.split_route_mode;
            }
            if (webForm.ssl === 'letsencrypt') {
                payload.le_challenge = webForm.le_challenge;
                if (webForm.le_email) payload.le_email = webForm.le_email;
            }
            if (webForm.frontend_domain) payload.frontend_domain = webForm.frontend_domain;
            if (webForm.admin_domain) payload.admin_domain = webForm.admin_domain;
            if (webForm.frontend_root) payload.frontend_root = webForm.frontend_root;
            if (webForm.frontend_cmd) payload.frontend_cmd = webForm.frontend_cmd;
            await api.patchMagentoStore(webStore.id, payload);
            const res = await api.applyMagentoWeb(webStore.id);
            toast.success(`Applied: ${(res.applied || []).join(', ')}`);
            setWebStore(null);
            load();
        } catch (err) {
            toast.error(err.message);
        } finally {
            setWebBusy(false);
        }
    };

    const loadVhost = async () => {
        const res = await api.getMagentoVhost(webStore.id).catch((e) => ({ content: `# ${e.message}` }));
        setVhostText(res.content || '');
    };

    const saveVhost = async () => {
        setWebBusy(true);
        try {
            await api.putMagentoVhost(webStore.id, vhostText, false);
            toast.success('vhost updated and nginx reloaded');
        } catch (err) {
            toast.error(err.message);
        } finally {
            setWebBusy(false);
        }
    };

    // ── Runtime: run user, PHP-FPM pool, php.ini, extensions, permissions ─
    const [runtimeStore, setRuntimeStore] = useState(null);
    const [runtimeData, setRuntimeData] = useState(null);
    const [runtimeForm, setRuntimeForm] = useState(null);
    const [runtimeBusy, setRuntimeBusy] = useState(false);

    const openRuntime = async (store) => {
        setRuntimeStore(store);
        setRuntimeData(null);
        const data = await api.getMagentoRuntime(store.id).catch((e) => {
            toast.error(e.message);
            return null;
        });
        if (!data) return;
        setRuntimeData(data);
        const ini = data.php?.ini?.settings || {};
        const pool = data.php?.pool || {};
        setRuntimeForm({
            run_user: data.store?.run_user || store.run_user || 'www-data',
            memory_limit: ini.memory_limit || data.php?.info?.memory_limit || '2G',
            max_execution_time: ini.max_execution_time || data.php?.info?.max_execution_time || '300',
            post_max_size: ini.post_max_size || data.php?.info?.post_max_size || '64M',
            upload_max_filesize: ini.upload_max_filesize || data.php?.info?.upload_max_filesize || '64M',
            opcache_memory: ini['opcache.memory_consumption'] || data.php?.info?.['opcache.memory_consumption'] || '256',
            max_children: pool.max_children || '10',
            install_extensions: '',
        });
    };

    const saveRuntime = async ({ repairOnly = false } = {}) => {
        if (!runtimeStore || !runtimeForm) return;
        setRuntimeBusy(true);
        try {
            const payload = repairOnly ? { repair_permissions: true } : {
                run_user: runtimeForm.run_user || 'www-data',
                pool_config: {
                    user: runtimeForm.run_user || 'www-data',
                    group: runtimeForm.run_user || 'www-data',
                    memory_limit: runtimeForm.memory_limit,
                    max_execution_time: runtimeForm.max_execution_time,
                    post_max_size: runtimeForm.post_max_size,
                    upload_max_filesize: runtimeForm.upload_max_filesize,
                    opcache_memory: runtimeForm.opcache_memory,
                    max_children: runtimeForm.max_children,
                    disable_functions: '',
                },
                ini: {
                    memory_limit: runtimeForm.memory_limit,
                    max_execution_time: runtimeForm.max_execution_time,
                    post_max_size: runtimeForm.post_max_size,
                    upload_max_filesize: runtimeForm.upload_max_filesize,
                    'opcache.memory_consumption': runtimeForm.opcache_memory,
                },
                repair_permissions: true,
            };
            const exts = (runtimeForm.install_extensions || '').split(',').map((x) => x.trim()).filter(Boolean);
            if (!repairOnly && exts.length) payload.install_extensions = exts;
            const res = await api.updateMagentoRuntime(runtimeStore.id, payload);
            toast.success(`Runtime updated: ${(res.applied || []).join(', ')}`);
            await openRuntime(runtimeStore);
            load();
        } catch (err) {
            toast.error(err.message);
        } finally {
            setRuntimeBusy(false);
        }
    };

    const frontendCtl = async (store, action) => {
        try {
            const res = await api.magentoFrontendAction(store.id, action);
            if (action === 'logs') {
                setActionOutput({ store: store.name, action: 'frontend logs', success: true, output: (res.lines || []).join('\n') });
            } else {
                toast.success(res.message || `frontend ${action} ok`);
            }
        } catch (err) {
            toast.error(`frontend ${action}: ${err.message}`);
        }
    };

    // ── DB Backups ──────────────────────────────────────────────────
    const [backupStore, setBackupStore] = useState(null);
    const [backups, setBackups] = useState([]);
    const [backupPolicy, setBackupPolicy] = useState({ schedule: 'none', retention: 7 });
    const [backupBusy, setBackupBusy] = useState(false);

    const openBackups = async (store) => {
        setBackupStore(store);
        setBackupPolicy({ schedule: store.backup_schedule || 'none', retention: store.backup_retention ?? 7 });
        const r = await api.listMagentoBackups(store.id).catch(() => ({ backups: [] }));
        setBackups(r.backups || []);
    };
    const refreshBackups = async () => {
        const r = await api.listMagentoBackups(backupStore.id).catch(() => ({ backups: [] }));
        setBackups(r.backups || []);
    };
    const runBackup = async () => {
        setBackupBusy(true);
        try { const r = await api.createMagentoBackup(backupStore.id); if (r.success) toast.success(`Backup ${r.size_human}`); else toast.error(r.error); await refreshBackups(); }
        catch (e) { toast.error(e.message); } finally { setBackupBusy(false); }
    };
    const restoreBackup = async (f) => {
        const ok = await confirm({ title: `Restore ${f}`, message: 'This overwrites the current database with the backup contents.' });
        if (!ok) return;
        try { const r = await api.restoreMagentoBackup(backupStore.id, f); toast.success(r.message || 'restored'); }
        catch (e) { toast.error(e.message); }
    };
    const deleteBackup = async (f) => {
        try { await api.deleteMagentoBackup(backupStore.id, f); await refreshBackups(); }
        catch (e) { toast.error(e.message); }
    };
    const savePolicy = async () => {
        try { await api.setMagentoBackupPolicy(backupStore.id, backupPolicy.schedule, Number(backupPolicy.retention) || 7); toast.success('Backup policy saved'); load(); }
        catch (e) { toast.error(e.message); }
    };

    const running = stores.filter((s) => s.status === 'running').length;
    const provisioning = stores.filter((s) => s.status === 'provisioning').length;

    return (
        <div className="space-y-6">
            <PageTopbar
                icon={<ShoppingBag size={18} />}
                title="Magento"
                actions={(
                    <div className="flex items-center gap-2">
                        <Button variant="outline" size="sm" onClick={load}>
                            <RefreshCw size={14} className="mr-1" /> Refresh
                        </Button>
                        <Button size="sm" onClick={() => setShowCreate(true)}>
                            <Plus size={14} className="mr-1" /> New Store
                        </Button>
                    </div>
                )}
            />

            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                <MetricCard icon={<ShoppingBag size={16} />} label="Stores" value={stores.length} />
                <MetricCard icon={<Activity size={16} />} label="Running" value={running} tone="green" />
                <MetricCard icon={<RefreshCw size={16} />} label="Provisioning" value={provisioning} tone={provisioning ? 'cyan' : 'accent'} />
                <MetricCard icon={<Zap size={16} />} label="Quick Actions" value={actions.length} />
            </div>

            {loading ? (
                <div className="text-muted-foreground text-sm">Loading stores…</div>
            ) : stores.length === 0 ? (
                <EmptyState
                    icon={ShoppingBag}
                    title="No Magento stores yet"
                    description="Create the Magento data-plane stack first, then optionally initialize Magento or attach your existing Magento and frontend source paths."
                    action={(
                        <Button onClick={() => setShowCreate(true)}>
                            <Plus size={14} className="mr-1" /> Create your first stack
                        </Button>
                    )}
                />
            ) : (
                <div className="grid gap-4">
                    {stores.map((store) => (
                        <div key={store.id} className="rounded-lg border bg-card p-4 space-y-3">
                            <div className="flex items-center justify-between flex-wrap gap-2">
                                <div className="flex items-center gap-3">
                                    <ShoppingBag size={18} className="text-primary" />
                                    <div>
                                        <div className="font-semibold flex items-center gap-2">
                                            {store.name}
                                            <Pill kind={STATUS_KIND[store.status] || 'gray'}>{store.status}</Pill>
                                        </div>
                                        <div className="text-xs text-muted-foreground flex items-center gap-1">
                                            <Globe size={11} />
                                            <a href={`http://${store.domain}/`} target="_blank" rel="noreferrer" className="hover:underline">
                                                {store.domain}
                                            </a>
                                            <span className="mx-1">·</span>
                                            Magento {store.magento_version} · PHP {store.php_version} · {store.distribution} · {store.install_magento ? 'initialized by ServerKit' : 'stack only/custom source'}
                                        </div>
                                    </div>
                                </div>
                                <div className="flex items-center gap-1.5 flex-wrap">
                                    {store.status === 'running' && PRIMARY_ACTIONS.map((a) => (
                                        <Button
                                            key={a.id}
                                            variant="outline"
                                            size="sm"
                                            disabled={busyAction === `${store.id}:${a.id}`}
                                            onClick={() => runAction(store, a.id)}
                                        >
                                            {busyAction === `${store.id}:${a.id}` ? <RefreshCw size={13} className="animate-spin" /> : a.label}
                                        </Button>
                                    ))}
                                    {store.status === 'running' && (
                                        <Button variant="outline" size="sm" onClick={() => openHealth(store)}>
                                            <Activity size={13} className="mr-1" /> Health
                                        </Button>
                                    )}
                                    {store.status === 'failed' && (
                                        <Button
                                            variant="outline"
                                            size="sm"
                                            onClick={() => retryProvision(store)}
                                            disabled={busyAction === `${store.id}:retry-provision`}
                                        >
                                            <RefreshCw size={13} className="mr-1" /> Retry provision
                                        </Button>
                                    )}
                                    {store.status === 'running' && (
                                        <Button variant="outline" size="sm" onClick={() => openWeb(store)}>
                                            <Globe size={13} className="mr-1" /> Web
                                        </Button>
                                    )}
                                    {store.status === 'running' && (
                                        <Button variant="outline" size="sm" onClick={() => openRuntime(store)}>
                                            <Zap size={13} className="mr-1" /> Runtime
                                        </Button>
                                    )}
                                    {store.status === 'running' && (
                                        <Button variant="outline" size="sm" onClick={() => openBackups(store)}>
                                            <Database size={13} className="mr-1" /> Backups
                                        </Button>
                                    )}
                                    {store.frontend_cmd && store.status === 'running' && (
                                        <>
                                            <Button variant="outline" size="sm" onClick={() => frontendCtl(store, 'restart')}>FE Restart</Button>
                                            <Button variant="outline" size="sm" onClick={() => frontendCtl(store, 'logs')}>FE Logs</Button>
                                        </>
                                    )}
                                    <Button variant="outline" size="sm" onClick={() => openLog(store)}>
                                        <ScrollText size={13} className="mr-1" /> Log
                                    </Button>
                                    <Button variant="outline" size="sm" className="text-destructive" onClick={() => removeStore(store)}>
                                        <Trash2 size={13} />
                                    </Button>
                                </div>
                            </div>

                            {store.status === 'provisioning' && (
                                <div className="text-xs text-muted-foreground flex items-center gap-2">
                                    <RefreshCw size={12} className="animate-spin" />
                                    {store.status_detail}
                                </div>
                            )}
                            {store.status === 'failed' && (
                                <div className="text-xs text-destructive">{store.status_detail}</div>
                            )}

                            <div className="text-xs text-muted-foreground flex items-center gap-4 flex-wrap">
                                {store.ssl_mode !== 'none' && (
                                    <span className="flex items-center gap-1">
                                        TLS: {store.ssl_mode === 'letsencrypt' ? 'LE' : 'self'}
                                        {store.ssl_cert_days != null && ` (${store.ssl_cert_days}d)`}
                                    </span>
                                )}
                                <span className="flex items-center gap-1"><Database size={11} /> db :{store.ports?.db}</span>
                                <span className="flex items-center gap-1"><SearchIcon size={11} /> opensearch :{store.ports?.search}</span>
                                <span>redis :{store.ports?.redis}</span>
                                <span>
                                    mail <a className="hover:underline" href={`http://127.0.0.1:${store.ports?.mail_ui}/`} target="_blank" rel="noreferrer">:{store.ports?.mail_ui}</a>
                                </span>
                                {store.admin_url && (
                                    <a href={store.admin_url} target="_blank" rel="noreferrer" className="flex items-center gap-1 hover:underline">
                                        <ExternalLink size={11} /> Admin
                                    </a>
                                )}
                                <span className="font-mono">{store.root_path}</span>
                            </div>
                        </div>
                    ))}
                </div>
            )}

            {actionOutput && (
                <div className="rounded-lg border bg-card p-4">
                    <div className="flex items-center justify-between mb-2">
                        <div className="text-sm font-semibold">
                            {actionOutput.store} · {actionOutput.action}
                            <Pill kind={actionOutput.success ? 'green' : 'red'} className="ml-2">
                                {actionOutput.success ? 'success' : 'failed'}
                            </Pill>
                        </div>
                        <Button variant="ghost" size="sm" onClick={() => setActionOutput(null)}>×</Button>
                    </div>
                    <pre className="text-xs bg-muted rounded p-3 max-h-64 overflow-auto whitespace-pre-wrap">
                        {(actionOutput.output || actionOutput.error || '').trim() || '(no output)'}
                    </pre>
                </div>
            )}

            {/* Create store modal */}
            <Modal open={showCreate} onClose={() => setShowCreate(false)} title="Create Magento stack" size="xl">
                <div className="space-y-5">
                    <div className="rounded-lg border border-amber-300/40 bg-amber-500/10 px-4 py-3 text-sm text-amber-900 dark:text-amber-100">
                        <strong>Stack creation is separate from Magento initialization.</strong> By default ServerKit only creates the data services and records your paths. Enable “Initialize Magento” when you want ServerKit to run Composer and <code>setup:install</code>.
                    </div>

                    <section className="rounded-lg border bg-card p-4 space-y-3">
                        <div>
                            <h3 className="text-sm font-semibold">1. Identity and paths</h3>
                            <p className="text-xs text-muted-foreground">Choose where ServerKit should keep stack files and, optionally, where your existing Magento source lives.</p>
                        </div>
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                            <div>
                                <Label>Store name</Label>
                                <Input placeholder="shop1" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
                                <p className="text-xs text-muted-foreground mt-1">lowercase, digits, hyphens</p>
                            </div>
                            <div>
                                <Label>{form.headless_mode === 'split' ? 'Primary Magento domain (unused in split)' : 'Primary Magento domain'}</Label>
                                <Input placeholder={form.headless_mode === 'split' ? 'leave blank for split mode' : 'shop.example.com'} value={form.domain} onChange={(e) => setForm({ ...form, domain: e.target.value })} />
                            </div>
                            <div>
                                <Label>Stack root path</Label>
                                <Input placeholder="/srv/serverkit/stacks/shop1" value={form.root_path} onChange={(e) => setForm({ ...form, root_path: e.target.value })} />
                                <p className="text-xs text-muted-foreground mt-1">Compose files, logs and generated stack state live here. Blank uses /srv/serverkit/stacks/&lt;name&gt;.</p>
                            </div>
                            <div>
                                <Label>Existing Magento source path</Label>
                                <Input placeholder="/home/ubuntu/mjsg/magento/current" value={form.magento_source_path} onChange={(e) => setForm({ ...form, magento_source_path: e.target.value })} />
                                <p className="text-xs text-muted-foreground mt-1">Use your own checkout/release. Blank uses &lt;stack root&gt;/src.</p>
                            </div>
                        </div>
                    </section>

                    <section className="rounded-lg border bg-card p-4 space-y-3">
                        <div className="flex items-start justify-between gap-4">
                            <div>
                                <h3 className="text-sm font-semibold">2. Magento runtime</h3>
                                <p className="text-xs text-muted-foreground">Magento version picks defaults only. Override PHP explicitly when your project needs a different installed PHP minor.</p>
                            </div>
                            <label className="flex items-center gap-2 rounded border px-3 py-2 text-sm cursor-pointer">
                                <input type="checkbox" checked={form.install_magento} onChange={(e) => setForm({ ...form, install_magento: e.target.checked })} />
                                Initialize Magento
                            </label>
                        </div>
                        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                            <div>
                                <Label>Magento version</Label>
                                <Select value={form.magento_version} onValueChange={(value) => { const selected = versions.find((v) => v.magento === value); setForm({ ...form, magento_version: value, php_version: form.php_version || selected?.php || '' }); }}>
                                    <SelectTrigger><SelectValue placeholder="Select Magento version" /></SelectTrigger>
                                    <SelectContent>
                                        {versions.map((v) => <SelectItem key={v.magento} value={v.magento}>{v.magento} (default PHP {v.php}, Composer {v.composer})</SelectItem>)}
                                    </SelectContent>
                                </Select>
                            </div>
                            <div>
                                <Label>PHP version</Label>
                                <Input placeholder={versions.find((v) => v.magento === form.magento_version)?.php || '8.3'} value={form.php_version} onChange={(e) => setForm({ ...form, php_version: e.target.value })} />
                                <p className="text-xs text-muted-foreground mt-1">Example: 8.4 or 8.3. ServerKit can auto-install it if missing.</p>
                            </div>
                            <div>
                                <Label>Distribution</Label>
                                <Select value={form.distribution} onValueChange={(value) => setForm({ ...form, distribution: value })}>
                                    <SelectTrigger><SelectValue /></SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="mage-os">Mage-OS mirror (no auth keys)</SelectItem>
                                        <SelectItem value="magento">Magento Open Source (repo.magento.com)</SelectItem>
                                    </SelectContent>
                                </Select>
                            </div>
                        </div>
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-3 text-sm">
                            <label className="flex items-center gap-2 rounded border px-3 py-2 cursor-pointer">
                                <input type="checkbox" checked={form.auto_install_php} onChange={(e) => setForm({ ...form, auto_install_php: e.target.checked })} />
                                Auto-install missing PHP/FPM on the host
                            </label>
                            <div>
                                <Label>PHP extension profile</Label>
                                <Select value={form.php_extension_profile} onValueChange={(value) => setForm({ ...form, php_extension_profile: value })}>
                                    <SelectTrigger><SelectValue /></SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="minimal">Minimal</SelectItem>
                                        <SelectItem value="magento">Magento required</SelectItem>
                                        <SelectItem value="all-supported">All supported</SelectItem>
                                    </SelectContent>
                                </Select>
                            </div>
                        </div>
                        {!form.install_magento && (
                            <p className="text-xs text-muted-foreground">Initialization is off: ServerKit will not run <code>composer create-project</code> or <code>bin/magento setup:install</code>. If no source path is provided, only the data services are started.</p>
                        )}
                    </section>

                    <section className="rounded-lg border bg-card p-4 space-y-3">
                        <div>
                            <h3 className="text-sm font-semibold">3. Data service images</h3>
                            <p className="text-xs text-muted-foreground">Use full image references, not just major versions. This supports Magento/PHP stack combinations where patch releases change service requirements.</p>
                        </div>
                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
                            {SERVICE_FIELDS.map((field) => {
                                const defaultImage = catalog.service_versions?.[field.id];
                                const selected = serviceSelectValue(field);
                                const isCustom = selected === CUSTOM_IMAGE_VALUE;
                                return (
                                    <div key={field.id} className="space-y-1.5">
                                        <Label>{field.label}</Label>
                                        <Select value={selected} onValueChange={(value) => handleServiceChoice(field, value)}>
                                            <SelectTrigger><SelectValue /></SelectTrigger>
                                            <SelectContent>
                                                <SelectItem value={DEFAULT_SERVICE_VALUE}>ServerKit default{defaultImage ? ` — ${defaultImage}` : ''}</SelectItem>
                                                {field.options.map(([value, label]) => (
                                                    <SelectItem key={value} value={value}>{label}</SelectItem>
                                                ))}
                                                <SelectItem value={CUSTOM_IMAGE_VALUE}>Custom image…</SelectItem>
                                            </SelectContent>
                                        </Select>
                                        {isCustom && (
                                            <Input
                                                className="mt-2"
                                                placeholder="registry.example.com/image:tag"
                                                value={serviceValue(field)}
                                                onChange={(e) => updateServiceVersion(field.id, e.target.value)}
                                            />
                                        )}
                                        <p className="text-xs text-muted-foreground">{field.helper}</p>
                                    </div>
                                );
                            })}
                        </div>
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-3 text-sm">
                            <label className="flex items-center gap-2 rounded border px-3 py-2 cursor-pointer">
                                <input type="checkbox" checked={form.use_rabbitmq} onChange={(e) => setForm({ ...form, use_rabbitmq: e.target.checked })} />
                                Include RabbitMQ service
                            </label>
                            <label className="flex items-center gap-2 rounded border px-3 py-2 cursor-pointer">
                                <input type="checkbox" checked={form.use_varnish} onChange={(e) => setForm({ ...form, use_varnish: e.target.checked })} />
                                Include Varnish FPC service
                            </label>
                        </div>
                    </section>

                    <section className="rounded-lg border bg-card p-4 space-y-3">
                        <div>
                            <h3 className="text-sm font-semibold">4. Web, headless and custom frontend</h3>
                            <p className="text-xs text-muted-foreground">Point ServerKit at your own Next.js/Nuxt/custom frontend path and command, or leave headless disabled.</p>
                        </div>
                        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                            <div>
                                <Label>Headless mode</Label>
                                <Select value={form.headless_mode} onValueChange={(value) => setForm({ ...form, headless_mode: value })}>
                                    <SelectTrigger><SelectValue /></SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="none">None / Magento storefront</SelectItem>
                                        <SelectItem value="shared">Shared domain (frontend at /, Magento routed)</SelectItem>
                                        <SelectItem value="separate">Separate domains</SelectItem>
                                        <SelectItem value="split">Split frontend + API + admin domains</SelectItem>
                                    </SelectContent>
                                </Select>
                            </div>
                            <div>
                                <Label>Frontend / Next.js path</Label>
                                <Input placeholder="/home/ubuntu/mjsg/frontend/current" value={form.frontend_root} onChange={(e) => setForm({ ...form, frontend_root: e.target.value })} />
                            </div>
                            <div>
                                <Label>Frontend command</Label>
                                <Input placeholder="/usr/bin/npm run start" value={form.frontend_cmd} onChange={(e) => setForm({ ...form, frontend_cmd: e.target.value })} />
                            </div>
                            <div>
                                <Label>Frontend port</Label>
                                <Input type="number" value={form.frontend_port} onChange={(e) => setForm({ ...form, frontend_port: e.target.value })} />
                                <p className="text-xs text-muted-foreground mt-1">0 = serve static export from frontend path.</p>
                            </div>
                            {(form.headless_mode === 'separate' || form.headless_mode === 'split') && (
                                <div>
                                    <Label>Frontend domain</Label>
                                    <Input placeholder="mjsg-dev.hcm1.smartosc.com" value={form.frontend_domain} onChange={(e) => setForm({ ...form, frontend_domain: e.target.value })} />
                                </div>
                            )}
                            {form.headless_mode === 'split' && (
                                <>
                                    <div>
                                        <Label>Magento API domain</Label>
                                        <Input placeholder="mjsgapi-dev.hcm1.smartosc.com" value={form.api_domain || ''} onChange={(e) => setForm({ ...form, api_domain: e.target.value })} />
                                    </div>
                                    <div>
                                        <Label>Admin domain</Label>
                                        <Input placeholder="mjsg-boservices.hcm1.smartosc.com" value={form.admin_domain || ''} onChange={(e) => setForm({ ...form, admin_domain: e.target.value })} />
                                    </div>
                                    <div>
                                        <Label>Split route policy</Label>
                                        <Select value={form.split_route_mode} onValueChange={(value) => setForm({ ...form, split_route_mode: value })}>
                                            <SelectTrigger><SelectValue /></SelectTrigger>
                                            <SelectContent>
                                                <SelectItem value="api_only">API/admin surfaces only</SelectItem>
                                                <SelectItem value="full_proxy">Full Magento proxy on API/admin domains</SelectItem>
                                            </SelectContent>
                                        </Select>
                                    </div>
                                </>
                            )}
                            {form.headless_mode === 'shared' && (
                                <div className="md:col-span-2">
                                    <Label>Magento routes on shared domain</Label>
                                    <Input placeholder="/checkout, /customer, /graphql" value={form.magento_routes} onChange={(e) => setForm({ ...form, magento_routes: e.target.value })} />
                                </div>
                            )}
                        </div>
                    </section>

                    <section className="rounded-lg border bg-card p-4 space-y-3">
                        <div>
                            <h3 className="text-sm font-semibold">5. TLS and process user</h3>
                        </div>
                        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                            <div>
                                <Label>TLS</Label>
                                <Select value={form.ssl} onValueChange={(value) => setForm({ ...form, ssl: value })}>
                                    <SelectTrigger><SelectValue /></SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="none">None (HTTP)</SelectItem>
                                        <SelectItem value="self-signed">Self-signed</SelectItem>
                                        <SelectItem value="letsencrypt">Let's Encrypt</SelectItem>
                                    </SelectContent>
                                </Select>
                            </div>
                            <div>
                                <Label>Run PHP/managed frontend as user</Label>
                                <Input placeholder="www-data" value={form.run_user} onChange={(e) => setForm({ ...form, run_user: e.target.value })} />
                            </div>
                            {form.ssl === 'letsencrypt' && (
                                <div>
                                    <Label>LE challenge</Label>
                                    <Select value={form.le_challenge} onValueChange={(value) => setForm({ ...form, le_challenge: value })}>
                                        <SelectTrigger><SelectValue /></SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="dns">DNS-01 (Cloudflare)</SelectItem>
                                            <SelectItem value="http">HTTP-01 (webroot)</SelectItem>
                                        </SelectContent>
                                    </Select>
                                </div>
                            )}
                            {form.ssl === 'letsencrypt' && (
                                <div className="md:col-span-2">
                                    <Label>Let's Encrypt email</Label>
                                    <Input placeholder="admin@example.com" value={form.le_email} onChange={(e) => setForm({ ...form, le_email: e.target.value })} />
                                </div>
                            )}
                        </div>
                    </section>

                    <div className="flex justify-end gap-2">
                        <Button variant="outline" onClick={() => setShowCreate(false)}>Cancel</Button>
                        <Button onClick={createStore} disabled={creating}>{creating ? 'Starting…' : (form.install_magento ? 'Create stack + initialize Magento' : 'Create stack only')}</Button>
                    </div>
                </div>
            </Modal>

            {/* One-time credentials modal */}
            <Modal open={!!credentials} onClose={() => setCredentials(null)} title="Stack credentials (shown once)" size="md">
                {credentials && (
                    <div className="space-y-3 text-sm">
                        <p className="text-muted-foreground text-xs">
                            Save these now — the panel masks them afterwards (admins can reveal via the API).
                        </p>
                        {[
                            ['Admin user', 'admin'],
                            ...(credentials.install_magento ? [['Admin password', credentials.admin_password]] : []),
                            ['DB password', credentials.db_password],
                        ].map(([label, value]) => (
                            <div key={label} className="flex items-center justify-between rounded border px-3 py-2">
                                <span className="text-muted-foreground">{label}</span>
                                <span className="font-mono flex items-center gap-2">
                                    {value}
                                    <Button
                                        variant="ghost" size="sm"
                                        onClick={() => { navigator.clipboard.writeText(value); toast.success('Copied'); }}
                                    >
                                        <Copy size={12} />
                                    </Button>
                                </span>
                            </div>
                        ))}
                        <div className="flex justify-end">
                            <Button onClick={() => setCredentials(null)}>Done</Button>
                        </div>
                    </div>
                )}
            </Modal>

            {/* Provision log modal */}
            <Modal open={!!logStore} onClose={() => setLogStore(null)} title={`Provision log — ${logStore?.name || ''}`} size="lg">
                <pre className="text-xs bg-muted rounded p-3 max-h-96 overflow-auto whitespace-pre-wrap">
                    {logLines.length ? logLines.join('\n') : 'No log yet.'}
                </pre>
                <div className="flex justify-end mt-3">
                    <Button variant="outline" size="sm" onClick={() => openLog(logStore)}>
                        <RefreshCw size={13} className="mr-1" /> Refresh
                    </Button>
                </div>
            </Modal>

            {/* Health modal */}
            <Modal open={!!healthStore} onClose={() => setHealthStore(null)} title={`Health — ${healthStore?.name || ''}`} size="md">
                {!health ? (
                    <div className="text-sm text-muted-foreground">Checking…</div>
                ) : (
                    <div className="space-y-4 text-sm">
                        <div>
                            <div className="font-semibold mb-1">Data plane</div>
                            <div className="grid grid-cols-2 gap-2">
                                {(health.services || []).map((s) => (
                                    <div key={s.service} className="flex items-center justify-between rounded border px-3 py-1.5">
                                        <span>{s.service}</span>
                                        <Pill kind={s.health === 'healthy' ? 'green' : 'amber'}>{s.health || s.state}</Pill>
                                    </div>
                                ))}
                            </div>
                        </div>
                        <div>
                            <div className="font-semibold mb-1">Cron (last hour)</div>
                            {health.cron?.available ? (
                                <div className="flex gap-2 flex-wrap">
                                    {Object.entries(health.cron.last_hour || {}).map(([k, v]) => (
                                        <Pill key={k} kind={k === 'success' ? 'green' : k === 'error' ? 'red' : 'gray'}>
                                            {k}: {v}
                                        </Pill>
                                    ))}
                                </div>
                            ) : (
                                <span className="text-muted-foreground text-xs">cron data unavailable</span>
                            )}
                        </div>
                        <div>
                            <div className="font-semibold mb-1">
                                Indexers ({(health.indexers || []).filter((i) => String(i.status).includes('Ready')).length}/{(health.indexers || []).length} ready)
                            </div>
                            <div className="max-h-48 overflow-auto space-y-1">
                                {(health.indexers || []).map((i) => (
                                    <div key={i.id} className="flex items-center justify-between text-xs rounded border px-2 py-1">
                                        <span>{i.title}</span>
                                        <Pill kind={String(i.status).includes('Ready') ? 'green' : 'amber'}>{i.status}</Pill>
                                    </div>
                                ))}
                            </div>
                        </div>
                    </div>
                )}
            </Modal>

            {/* Web Config: PATCH web fields + apply-web, plus editable vhost */}
            <Modal open={!!webStore} onClose={() => setWebStore(null)} title={`Web config — ${webStore?.name || ''}`} size="lg">
                {webForm && (
                    <div className="space-y-4">
                        <div className="flex gap-2 border-b">
                            <button className={`px-3 py-1.5 text-sm ${webTab === 'config' ? 'border-b-2 border-primary font-medium' : 'text-muted-foreground'}`} onClick={() => setWebTab('config')}>Config</button>
                            <button className={`px-3 py-1.5 text-sm ${webTab === 'vhost' ? 'border-b-2 border-primary font-medium' : 'text-muted-foreground'}`} onClick={() => { setWebTab('vhost'); if (!vhostText) loadVhost(); }}>Edit nginx vhost</button>
                        </div>

                        {webTab === 'config' ? (
                            <div className="space-y-3">
                                <div className="grid grid-cols-2 gap-3">
                                    <div>
                                        <Label>SSL</Label>
                                        <Select value={webForm.ssl} onValueChange={(value) => setWebForm({ ...webForm, ssl: value })}>
                                            <SelectTrigger><SelectValue /></SelectTrigger>
                                            <SelectContent>
                                                <SelectItem value="none">None (HTTP)</SelectItem>
                                                <SelectItem value="self-signed">HTTPS (self-signed, multi-domain SAN)</SelectItem>
                                                <SelectItem value="letsencrypt">Let's Encrypt</SelectItem>
                                            </SelectContent>
                                        </Select>
                                        {certDays != null && (
                                            <p className="text-xs text-muted-foreground mt-1">cert expires in {certDays} days</p>
                                        )}
                                    </div>
                                    <div>
                                        <Label>Headless mode</Label>
                                        <Select value={webForm.headless_mode} onValueChange={(value) => setWebForm({ ...webForm, headless_mode: value })}>
                                            <SelectTrigger><SelectValue /></SelectTrigger>
                                            <SelectContent>
                                                <SelectItem value="none">None</SelectItem>
                                                <SelectItem value="shared">Shared</SelectItem>
                                                <SelectItem value="separate">Separate</SelectItem>
                                                <SelectItem value="split">Split (FE + API + admin)</SelectItem>
                                            </SelectContent>
                                        </Select>
                                    </div>
                                </div>
                                {webForm.headless_mode !== 'none' && (
                                    <>
                                        <div className="grid grid-cols-2 gap-3">
                                            {webForm.headless_mode !== 'shared' && (
                                                <div>
                                                    <Label>Frontend domain</Label>
                                                    <Input value={webForm.frontend_domain} onChange={(e) => setWebForm({ ...webForm, frontend_domain: e.target.value })} />
                                                </div>
                                            )}
                                            {webForm.headless_mode === 'split' && (
                                                <>
                                                    <div>
                                                        <Label>Magento API domain</Label>
                                                        <Input value={webForm.api_domain} onChange={(e) => setWebForm({ ...webForm, api_domain: e.target.value })} />
                                                    </div>
                                                    <div>
                                                        <Label>Admin domain</Label>
                                                        <Input value={webForm.admin_domain} onChange={(e) => setWebForm({ ...webForm, admin_domain: e.target.value })} />
                                                    </div>
                                                    <div>
                                                        <Label>Split route policy</Label>
                                                        <Select value={webForm.split_route_mode} onValueChange={(value) => setWebForm({ ...webForm, split_route_mode: value })}>
                                                            <SelectTrigger><SelectValue /></SelectTrigger>
                                                            <SelectContent>
                                                                <SelectItem value="api_only">API/admin surfaces only</SelectItem>
                                                                <SelectItem value="full_proxy">Full Magento proxy on API/admin domains</SelectItem>
                                                            </SelectContent>
                                                        </Select>
                                                    </div>
                                                </>
                                            )}
                                        </div>
                                        <div className="grid grid-cols-3 gap-3">
                                            <div>
                                                <Label>Frontend port</Label>
                                                <Input type="number" value={webForm.frontend_port} onChange={(e) => setWebForm({ ...webForm, frontend_port: e.target.value })} />
                                            </div>
                                            <div>
                                                <Label>Frontend folder</Label>
                                                <Input placeholder="/home/ubuntu/mjsg/frontend/current" value={webForm.frontend_root} onChange={(e) => setWebForm({ ...webForm, frontend_root: e.target.value })} />
                                            </div>
                                            {webForm.headless_mode === 'shared' && (
                                                <div>
                                                    <Label>Extra routes</Label>
                                                    <Input placeholder="/checkout" value={webForm.magento_routes} onChange={(e) => setWebForm({ ...webForm, magento_routes: e.target.value })} />
                                                </div>
                                            )}
                                        </div>
                                        <div>
                                            <Label>Managed frontend command (systemd)</Label>
                                            <Input placeholder="/usr/bin/node server.js" value={webForm.frontend_cmd} onChange={(e) => setWebForm({ ...webForm, frontend_cmd: e.target.value })} />
                                            <p className="text-xs text-muted-foreground mt-1">Absolute path, no shell operators. Runs as serverkit-fe-{webStore?.name}.service.</p>
                                        </div>
                                    </>
                                )}
                                {webForm.ssl === 'letsencrypt' && (
                                    <div className="grid grid-cols-2 gap-3">
                                        <div>
                                            <Label>LE challenge</Label>
                                            <Select value={webForm.le_challenge} onValueChange={(value) => setWebForm({ ...webForm, le_challenge: value })}>
                                                <SelectTrigger><SelectValue /></SelectTrigger>
                                                <SelectContent>
                                                    <SelectItem value="dns">DNS-01 (Cloudflare)</SelectItem>
                                                    <SelectItem value="http">HTTP-01 (webroot)</SelectItem>
                                                </SelectContent>
                                            </Select>
                                        </div>
                                        <div>
                                            <Label>LE email</Label>
                                            <Input value={webForm.le_email} onChange={(e) => setWebForm({ ...webForm, le_email: e.target.value })} />
                                        </div>
                                    </div>
                                )}
                                <div>
                                    <Label>Run as user</Label>
                                    <Input value={webForm.run_user} onChange={(e) => setWebForm({ ...webForm, run_user: e.target.value })} />
                                </div>
                                <div className="flex justify-end gap-2">
                                    {webForm.ssl === 'letsencrypt' && (
                                        <Button variant="outline" onClick={async () => {
                                            try { const r = await api.renewMagentoCert(webStore.id); toast.success(r.renewed ? `Renewed (${r.days_after}d)` : `No renewal: ${r.reason || r.days_remaining + 'd left'}`); load(); }
                                            catch (e) { toast.error(e.message); }
                                        }}>Renew cert</Button>
                                    )}
                                    <Button variant="outline" onClick={() => setWebStore(null)}>Cancel</Button>
                                    <Button onClick={saveAndApply} disabled={webBusy}>{webBusy ? 'Applying…' : 'Save & Apply'}</Button>
                                </div>
                            </div>
                        ) : (
                            <div className="space-y-3">
                                <textarea className="w-full h-80 font-mono text-xs rounded border bg-muted p-3" value={vhostText} onChange={(e) => setVhostText(e.target.value)} spellCheck={false} />
                                <p className="text-xs text-muted-foreground">Saved with nginx -t validation — a broken config is rolled back automatically.</p>
                                <div className="flex justify-end gap-2">
                                    <Button variant="outline" onClick={loadVhost}>Reload</Button>
                                    <Button onClick={saveVhost} disabled={webBusy}>{webBusy ? 'Saving…' : 'Save vhost'}</Button>
                                </div>
                            </div>
                        )}
                    </div>
                )}
            </Modal>
            {/* Runtime: PHP-FPM, php.ini, extensions, permissions */}
            <Modal open={!!runtimeStore} onClose={() => setRuntimeStore(null)} title={`Runtime — ${runtimeStore?.name || ''}`} size="lg">
                {!runtimeData || !runtimeForm ? (
                    <div className="text-sm text-muted-foreground">Loading runtime…</div>
                ) : (
                    <div className="space-y-4">
                        <div className="rounded-lg border bg-muted/30 p-3 text-xs text-muted-foreground space-y-1">
                            <div><strong>PHP:</strong> {runtimeData.php?.version} · <strong>FPM:</strong> {runtimeData.php?.fpm_status?.status}</div>
                            <div><strong>Pool:</strong> {runtimeData.php?.pool_name} · <strong>Socket:</strong> {runtimeData.php?.pool?.listen || 'not created yet'}</div>
                            <div><strong>Source:</strong> <span className="font-mono">{runtimeData.paths?.magento_source_path}</span></div>
                        </div>
                        <section className="rounded-lg border bg-card p-4 space-y-3">
                            <div>
                                <h3 className="text-sm font-semibold">User and PHP-FPM pool</h3>
                                <p className="text-xs text-muted-foreground">For dev, set run user to <code>ubuntu</code>. ServerKit updates the store pool and repairs nginx/PHP ACLs.</p>
                            </div>
                            <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                                <div>
                                    <Label>Run user/group</Label>
                                    <Input value={runtimeForm.run_user} onChange={(e) => setRuntimeForm({ ...runtimeForm, run_user: e.target.value })} />
                                </div>
                                <div>
                                    <Label>FPM max children</Label>
                                    <Input value={runtimeForm.max_children} onChange={(e) => setRuntimeForm({ ...runtimeForm, max_children: e.target.value })} />
                                </div>
                                <div>
                                    <Label>Memory limit</Label>
                                    <Input value={runtimeForm.memory_limit} onChange={(e) => setRuntimeForm({ ...runtimeForm, memory_limit: e.target.value })} />
                                </div>
                            </div>
                        </section>
                        <section className="rounded-lg border bg-card p-4 space-y-3">
                            <div>
                                <h3 className="text-sm font-semibold">php.ini overrides</h3>
                                <p className="text-xs text-muted-foreground">Writes ServerKit-managed overrides to PHP CLI and FPM conf.d, then restarts FPM.</p>
                            </div>
                            <div className="grid grid-cols-1 md:grid-cols-4 gap-3">
                                <div>
                                    <Label>Max execution</Label>
                                    <Input value={runtimeForm.max_execution_time} onChange={(e) => setRuntimeForm({ ...runtimeForm, max_execution_time: e.target.value })} />
                                </div>
                                <div>
                                    <Label>Post max size</Label>
                                    <Input value={runtimeForm.post_max_size} onChange={(e) => setRuntimeForm({ ...runtimeForm, post_max_size: e.target.value })} />
                                </div>
                                <div>
                                    <Label>Upload max filesize</Label>
                                    <Input value={runtimeForm.upload_max_filesize} onChange={(e) => setRuntimeForm({ ...runtimeForm, upload_max_filesize: e.target.value })} />
                                </div>
                                <div>
                                    <Label>OPcache memory</Label>
                                    <Input value={runtimeForm.opcache_memory} onChange={(e) => setRuntimeForm({ ...runtimeForm, opcache_memory: e.target.value })} />
                                </div>
                            </div>
                        </section>
                        <section className="rounded-lg border bg-card p-4 space-y-3">
                            <div>
                                <h3 className="text-sm font-semibold">Extensions and permissions</h3>
                                <p className="text-xs text-muted-foreground">Install selected supported PHP extensions or repair source ownership/ACLs for nginx and PHP-FPM.</p>
                            </div>
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                                <div>
                                    <Label>Install extensions</Label>
                                    <Input placeholder="redis, igbinary" value={runtimeForm.install_extensions} onChange={(e) => setRuntimeForm({ ...runtimeForm, install_extensions: e.target.value })} />
                                    <p className="text-xs text-muted-foreground mt-1">Comma-separated. Supported: {(runtimeData.php?.supported_extensions || []).join(', ')}</p>
                                </div>
                                <div>
                                    <Label>Installed extensions</Label>
                                    <div className="min-h-9 rounded border bg-muted px-3 py-2 text-xs text-muted-foreground max-h-24 overflow-auto">
                                        {(runtimeData.php?.extensions || []).map((ext) => ext.name).join(', ') || 'none reported'}
                                    </div>
                                </div>
                            </div>
                        </section>
                        <div className="flex justify-end gap-2">
                            <Button variant="outline" onClick={() => saveRuntime({ repairOnly: true })} disabled={runtimeBusy}>Repair permissions only</Button>
                            <Button variant="outline" onClick={() => setRuntimeStore(null)}>Cancel</Button>
                            <Button onClick={() => saveRuntime()} disabled={runtimeBusy}>{runtimeBusy ? 'Applying…' : 'Save runtime'}</Button>
                        </div>
                    </div>
                )}
            </Modal>

            {/* DB Backups */}
            <Modal open={!!backupStore} onClose={() => setBackupStore(null)} title={`Database backups — ${backupStore?.name || ''}`} size="lg">
                <div className="space-y-4">
                    <div className="flex items-center justify-between gap-3 flex-wrap">
                        <div className="flex items-end gap-2">
                            <div>
                                <Label>Schedule</Label>
                                <Select value={backupPolicy.schedule} onValueChange={(value) => setBackupPolicy({ ...backupPolicy, schedule: value })}>
                                    <SelectTrigger className="w-40"><SelectValue /></SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="none">Manual only</SelectItem>
                                        <SelectItem value="hourly">Hourly</SelectItem>
                                        <SelectItem value="daily">Daily</SelectItem>
                                        <SelectItem value="weekly">Weekly</SelectItem>
                                    </SelectContent>
                                </Select>
                            </div>
                            <div>
                                <Label>Keep</Label>
                                <Input type="number" className="w-20" value={backupPolicy.retention}
                                    onChange={(e) => setBackupPolicy({ ...backupPolicy, retention: e.target.value })} />
                            </div>
                            <Button variant="outline" size="sm" onClick={savePolicy}>Save policy</Button>
                        </div>
                        <Button size="sm" onClick={runBackup} disabled={backupBusy}>
                            {backupBusy ? 'Backing up…' : 'Back up now'}
                        </Button>
                    </div>
                    <div className="max-h-80 overflow-auto space-y-1">
                        {backups.length === 0 ? (
                            <div className="text-sm text-muted-foreground">No backups yet.</div>
                        ) : backups.map((b) => (
                            <div key={b.filename} className="flex items-center justify-between text-xs rounded border px-3 py-2">
                                <span className="font-mono">{b.filename}</span>
                                <span className="flex items-center gap-3">
                                    <span className="text-muted-foreground">{b.size_human} · {b.created_at}</span>
                                    <Button variant="outline" size="sm" onClick={() => restoreBackup(b.filename)}>Restore</Button>
                                    <Button variant="outline" size="sm" className="text-destructive" onClick={() => deleteBackup(b.filename)}>Delete</Button>
                                </span>
                            </div>
                        ))}
                    </div>
                </div>
            </Modal>
        </div>
    );
};

export default Magento;
