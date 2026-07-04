import { useState, useEffect, useCallback } from 'react';
import {
    Activity,
    Archive,
    CheckCircle2,
    DownloadCloud,
    FileArchive,
    Filter,
    FolderOpen,
    Globe2,
    LayoutGrid,
    Package,
    PackageCheck,
    Plug,
    PlugZap,
    Search,
    ServerCog,
    ShieldCheck,
    Sparkles,
    UploadCloud,
} from 'lucide-react';
import api from '../services/api';
import { useToast } from '../contexts/ToastContext';
import { sanitizeSvgInner } from '../utils/sanitizeSvg';
import Modal from '@/components/Modal';
import PageLoader from '../components/PageLoader';
import EmptyState from '../components/EmptyState';
import { StatStrip, Stat } from '../components/StatCard';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import { useTopbarActions } from '@/hooks/useTopbarActions';

const CATEGORIES = ['ai', 'monitoring', 'security', 'deployment', 'integration', 'ui', 'utility'];

const CATEGORY_ICONS = {
    ai: Sparkles,
    monitoring: Activity,
    security: ShieldCheck,
    deployment: ServerCog,
    integration: Plug,
    ui: LayoutGrid,
    utility: Package,
};

const PLUGIN_INSTALL_SOURCES = [
    { id: 'url', label: 'URL', icon: Globe2 },
    { id: 'path', label: 'Folder', icon: FolderOpen },
    { id: 'upload', label: 'Zip', icon: FileArchive },
];

const titleCase = (value = '') => {
    const cleaned = String(value || 'utility').replace(/[-_]/g, ' ');
    return cleaned
        .split(' ')
        .filter(Boolean)
        .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
        .join(' ');
};

const getCategoryIcon = (category) => CATEGORY_ICONS[category] || Package;

// A first-party ("by ServerKit") entry is one authored by ServerKit or one whose
// manifest explicitly opts in via `first_party`. Case-insensitive on author.
const isFirstParty = (author, source = {}) => {
    if (source && source.first_party) return true;
    return String(author || '').trim().toLowerCase() === 'serverkit';
};

// Union of categories actually present across the merged catalog, ordered by the
// canonical CATEGORIES list first, then any extras alphabetically (stable, deduped).
const deriveCatalogCategories = (entries) => {
    const present = new Set(entries.map((entry) => entry.category || 'utility'));
    const known = CATEGORIES.filter((item) => present.has(item));
    const extra = [...present].filter((item) => !CATEGORIES.includes(item)).sort();
    return [...known, ...extra];
};

const getRegistryCatalogEntry = (entry) => ({
    key: `registry:${entry.slug}`,
    source: 'registry',
    sourceLabel: 'Registry',
    sourceDetail: 'Remote registry package',
    installKey: entry.slug,
    displayName: entry.display_name || entry.slug,
    description: entry.description || 'No description provided.',
    category: entry.category || 'utility',
    version: entry.version || '0.0.0',
    author: entry.author,
    firstParty: isFirstParty(entry.author, entry),
    icon: entry.icon || null,
    screenshots: Array.isArray(entry.screenshots) ? entry.screenshots : [],
    permissions: Array.isArray(entry.permissions) ? entry.permissions : [],
    configSchema: entry.config_schema && typeof entry.config_schema === 'object' ? entry.config_schema : null,
    extensionType: 'registry',
    installed: Boolean(entry.installed),
    status: entry.status,
});

// Source badge tint: built-in is 'warning', registry is 'info'.
const sourceBadgeVariant = (source) => {
    if (source === 'local') return 'warning';
    if (source === 'registry') return 'info';
    return 'outline';
};

const getLocalCatalogEntry = (builtin) => {
    const manifest = builtin.manifest || {};

    return {
        key: `local:${builtin.slug}`,
        source: 'local',
        sourceLabel: 'Built-in',
        sourceDetail: 'Bundled with ServerKit',
        installKey: builtin.slug,
        displayName: manifest.display_name || builtin.slug,
        description: manifest.description || 'Bundled extension.',
        category: manifest.category || 'utility',
        version: manifest.version || '0.0.0',
        author: manifest.author,
        firstParty: isFirstParty(manifest.author, manifest),
        icon: manifest.icon || null,
        screenshots: Array.isArray(manifest.screenshots) ? manifest.screenshots : [],
        permissions: Array.isArray(manifest.permissions) ? manifest.permissions : [],
        configSchema: manifest.config_schema && typeof manifest.config_schema === 'object' ? manifest.config_schema : null,
        extensionType: 'built-in',
        installed: Boolean(builtin.installed),
        status: builtin.status,
    };
};

const catalogEntryMatches = (entry, search, category) => {
    if (category && entry.category !== category) return false;

    const query = search.trim().toLowerCase();
    if (!query) return true;

    return [
        entry.displayName,
        entry.description,
        entry.category,
        entry.author,
        entry.sourceLabel,
    ].some((value) => String(value || '').toLowerCase().includes(query));
};

const Marketplace = () => {
    const toast = useToast();
    const [plugins, setPlugins] = useState([]);
    const [builtins, setBuiltins] = useState([]);
    const [registryExtensions, setRegistryExtensions] = useState([]);
    const [pluginUpdates, setPluginUpdates] = useState([]);
    const [loading, setLoading] = useState(true);
    const [search, setSearch] = useState('');
    const [category, setCategory] = useState('');
    const [activeTab, setActiveTab] = useState('browse');
    const [pluginUrl, setPluginUrl] = useState('');
    const [pluginPath, setPluginPath] = useState('');
    const [pluginFile, setPluginFile] = useState(null);
    const [installSource, setInstallSource] = useState('url');
    const [installing, setInstalling] = useState(false);
    const [detailEntry, setDetailEntry] = useState(null);
    // Plugin pending uninstall — drives the keep-vs-purge data-policy dialog.
    const [uninstallTarget, setUninstallTarget] = useState(null);
    // Plugin whose config is being edited (#49) — drives the config dialog.
    const [configTarget, setConfigTarget] = useState(null);

    const loadExtensions = useCallback(async () => {
        try {
            const [pData, bData, rData, uData] = await Promise.all([
                api.getInstalledPlugins().catch(() => ({ plugins: [] })),
                api.getBuiltinExtensions().catch(() => ({ builtin: [] })),
                api.getRegistryExtensions().catch(() => ({ extensions: [] })),
                api.getPluginUpdates().catch(() => ({ updates: [] })),
            ]);
            setPlugins(pData.plugins || []);
            setBuiltins(bData.builtin || []);
            setRegistryExtensions(rData.extensions || []);
            setPluginUpdates(uData.updates || []);
        } catch {
            toast.error('Failed to load extensions');
        } finally {
            setLoading(false);
        }
    }, [toast]);

    useEffect(() => { loadExtensions(); }, [loadExtensions]);

    const handleBuiltinInstall = async (slug) => {
        setInstalling(true);
        try {
            const result = await api.installBuiltinExtension(slug);
            toast.success(`Installed "${result.display_name}". Hot-reload should pick it up; restart backend if blueprint routes do not appear.`);
            loadExtensions();
        } catch (err) {
            toast.error(err.message || 'Local install failed');
        } finally {
            setInstalling(false);
        }
    };

    const handleRegistryInstall = async (slug) => {
        setInstalling(true);
        try {
            const result = await api.installRegistryExtension(slug);
            toast.success(`Installed "${result.display_name}". Restart backend if blueprint routes do not appear.`);
            loadExtensions();
        } catch (err) {
            toast.error(err.message || 'Registry install failed');
        } finally {
            setInstalling(false);
        }
    };

    const installEntry = (entry) => {
        if (entry.source === 'local') {
            handleBuiltinInstall(entry.installKey);
        } else {
            handleRegistryInstall(entry.installKey);
        }
    };

    const handlePluginInstall = async () => {
        let action;
        if (installSource === 'url') {
            if (!pluginUrl.trim()) return;
            action = () => api.installPlugin(pluginUrl.trim());
        } else if (installSource === 'path') {
            if (!pluginPath.trim()) return;
            action = () => api.installPluginFromPath(pluginPath.trim());
        } else if (installSource === 'upload') {
            if (!pluginFile) return;
            action = () => api.installPluginFromZip(pluginFile);
        } else {
            return;
        }

        setInstalling(true);
        try {
            const result = await action();
            toast.success(`Plugin "${result.display_name}" installed. Restart backend to activate routes.`);
            setPluginUrl('');
            setPluginPath('');
            setPluginFile(null);
            loadExtensions();
        } catch (err) {
            toast.error(err.message || 'Plugin installation failed');
        } finally {
            setInstalling(false);
        }
    };

    // Open the data-policy dialog instead of uninstalling immediately, so the
    // operator can choose to keep or purge the extension's tables.
    const requestPluginUninstall = (plugin) => setUninstallTarget(plugin);

    const confirmPluginUninstall = async (purge) => {
        const plugin = uninstallTarget;
        setUninstallTarget(null);
        if (!plugin) return;
        try {
            await api.uninstallPlugin(plugin.id, purge);
            toast.success(purge ? 'Plugin uninstalled; data purged' : 'Plugin uninstalled; data kept');
            loadExtensions();
        } catch (err) { toast.error(err.message); }
    };

    const handlePluginUpdate = async (pluginId) => {
        setInstalling(true);
        try {
            const result = await api.updatePlugin(pluginId);
            toast.success(`Plugin "${result.display_name}" updated to v${result.version}.`);
            loadExtensions();
        } catch (err) {
            toast.error(err.message || 'Plugin update failed');
        } finally {
            setInstalling(false);
        }
    };

    const handlePluginToggle = async (plugin) => {
        try {
            if (plugin.status === 'active') {
                await api.disablePlugin(plugin.id);
                toast.success('Plugin disabled');
            } else {
                await api.enablePlugin(plugin.id);
                toast.success('Plugin enabled');
            }
            loadExtensions();
        } catch (err) { toast.error(err.message); }
    };

    const resetFilters = () => {
        setSearch('');
        setCategory('');
    };

    const openZipInstaller = () => {
        setInstallSource('upload');
        setActiveTab('plugins');
    };

    const pluginStatusVariant = (status) => {
        if (status === 'active') return 'success';
        if (status === 'error') return 'destructive';
        return 'outline';
    };

    useTopbarActions(() =>
        <>
            <Button variant="outline" size="sm" onClick={openZipInstaller}>
                <UploadCloud aria-hidden="true" />
                Import ZIP
            </Button>
        </>,
        [],
    );

    if (loading) return <PageLoader />;

    const localCatalogEntries = builtins.map(getLocalCatalogEntry);
    const registryCatalogEntries = registryExtensions.map(getRegistryCatalogEntry);
    const installedCatalogEntries = localCatalogEntries.filter((entry) => entry.installed);
    const installedBuiltinCount = installedCatalogEntries.length;
    const activePluginCount = plugins.filter((plugin) => plugin.status === 'active').length;
    const pluginIssueCount = plugins.filter((plugin) => plugin.status === 'error').length;
    const availableCount = builtins.length + registryExtensions.length;
    const installedCatalogCount = installedCatalogEntries.length;
    // Update descriptors keyed by both plugin_id and slug so PluginRow can match
    // whichever identifier it has on hand.
    const updatesByKey = new Map();
    pluginUpdates.forEach((update) => {
        if (update.plugin_id != null) updatesByKey.set(String(update.plugin_id), update);
        if (update.slug) updatesByKey.set(update.slug, update);
    });
    const mergedCatalogEntries = [...localCatalogEntries, ...registryCatalogEntries];
    const catalogCategories = deriveCatalogCategories(mergedCatalogEntries);
    const catalogEntries = mergedCatalogEntries
        .filter((entry) => catalogEntryMatches(entry, search, category));
    const hasFilters = Boolean(search.trim() || category);

    return (
        <div className="sk-tabgroup__inner marketplace-page">
            <StatStrip ariaLabel="Marketplace summary">
                <Stat label="Catalog" value={availableCount} />
                <Stat label="Built-in" value={builtins.length} />
                <Stat label="Installed" value={installedCatalogCount} />
                <Stat
                    label="Active Plugins"
                    value={`${activePluginCount}/${plugins.length}`}
                    state={pluginIssueCount > 0 ? 'danger' : undefined}
                />
            </StatStrip>

            <Tabs value={activeTab} onValueChange={setActiveTab} className="marketplace-tabs">
                <TabsList className="marketplace-tabs__list">
                    <TabsTrigger value="browse">
                        <LayoutGrid aria-hidden="true" />
                        Browse
                    </TabsTrigger>
                    <TabsTrigger value="installed">
                        <PackageCheck aria-hidden="true" />
                        Installed ({installedCatalogCount})
                    </TabsTrigger>
                    <TabsTrigger value="plugins">
                        <PlugZap aria-hidden="true" />
                        ServerKit Plugins ({plugins.length})
                    </TabsTrigger>
                </TabsList>

                <TabsContent value="browse">
                    <div className="marketplace-toolbar">
                        <div className="marketplace-search">
                            <Search className="marketplace-search__icon" aria-hidden="true" />
                            <Input
                                placeholder="Search extensions..."
                                value={search}
                                onChange={(event) => setSearch(event.target.value)}
                                aria-label="Search extensions"
                            />
                        </div>
                        <select
                            className="form-select marketplace-category-select"
                            value={category}
                            onChange={(event) => setCategory(event.target.value)}
                            aria-label="Filter by category"
                        >
                            <option value="">All Categories</option>
                            {catalogCategories.map((item) => (
                                <option key={item} value={item}>{titleCase(item)}</option>
                            ))}
                        </select>
                        {hasFilters && (
                            <Button variant="ghost" size="sm" onClick={resetFilters}>
                                Reset
                            </Button>
                        )}
                    </div>

                    <div className="cat-chips" role="group" aria-label="Filter by category">
                        <button
                            type="button"
                            className={`cat-chip ${category === '' ? 'cat-chip--active' : ''}`}
                            aria-pressed={category === ''}
                            onClick={() => setCategory('')}
                        >
                            All
                        </button>
                        {catalogCategories.map((item) => (
                            <button
                                key={item}
                                type="button"
                                className={`cat-chip ${category === item ? 'cat-chip--active' : ''}`}
                                aria-pressed={category === item}
                                onClick={() => setCategory(item)}
                            >
                                {titleCase(item)}
                            </button>
                        ))}
                    </div>

                    <div className="marketplace-browse-grid">
                        <div className="marketplace-main-stack">
                            <section className="marketplace-section">
                                <SectionHeader
                                    kicker="Catalog"
                                    title="Extension catalog"
                                    meta={`${catalogEntries.length} results`}
                                />
                                {catalogEntries.length > 0 ? (
                                    <div className="extensions-grid">
                                        {catalogEntries.map((entry) => (
                                            <CatalogExtensionCard
                                                key={entry.key}
                                                entry={entry}
                                                installing={installing}
                                                onInstall={
                                                    entry.source === 'local'
                                                        ? handleBuiltinInstall
                                                        : handleRegistryInstall
                                                }
                                                onOpenDetail={setDetailEntry}
                                                statusVariant={pluginStatusVariant}
                                            />
                                        ))}
                                    </div>
                                ) : (
                                    <EmptyState
                                        icon={Package}
                                        title="No catalog entries found"
                                        description={hasFilters ? 'No built-in or registry entries match the current filter.' : 'No extension entries are available yet.'}
                                    />
                                )}
                            </section>
                        </div>

                        <aside className="marketplace-side-panel" aria-label="Marketplace controls">
                            <div className="marketplace-panel">
                                <div className="marketplace-panel__title">
                                    <Filter aria-hidden="true" />
                                    Categories
                                </div>
                                <div className="marketplace-category-list">
                                    <button
                                        type="button"
                                        className={`marketplace-category ${category === '' ? 'marketplace-category--active' : ''}`}
                                        onClick={() => setCategory('')}
                                    >
                                        All
                                    </button>
                                    {catalogCategories.map((item) => {
                                        const Icon = getCategoryIcon(item);
                                        return (
                                            <button
                                                key={item}
                                                type="button"
                                                className={`marketplace-category marketplace-category--${item} ${category === item ? 'marketplace-category--active' : ''}`}
                                                onClick={() => setCategory(item)}
                                            >
                                                <Icon aria-hidden="true" />
                                                {titleCase(item)}
                                            </button>
                                        );
                                    })}
                                </div>
                            </div>

                            <div className="marketplace-panel">
                                <div className="marketplace-panel__title">
                                    <ServerCog aria-hidden="true" />
                                    Runtime
                                </div>
                                <div className="marketplace-runtime">
                                    <RuntimeRow label="Registry" value={registryExtensions.length} />
                                    <RuntimeRow label="Built-in" value={builtins.length} />
                                    <RuntimeRow label="Built-in installed" value={`${installedBuiltinCount}/${builtins.length}`} />
                                    <RuntimeRow label="Active plugins" value={`${activePluginCount}/${plugins.length}`} />
                                    <RuntimeRow
                                        label="Plugin issues"
                                        value={pluginIssueCount}
                                        danger={pluginIssueCount > 0}
                                    />
                                </div>
                            </div>
                        </aside>
                    </div>
                </TabsContent>

                <TabsContent value="installed">
                    <section className="marketplace-section">
                        <SectionHeader
                            kicker="Installed"
                            title="Installed extensions"
                            meta={`${installedCatalogCount} installed`}
                        />
                        {installedCatalogEntries.length > 0 ? (
                            <div className="installed-list">
                                {installedCatalogEntries.map((entry) => (
                                    <InstalledCatalogRow key={entry.key} entry={entry} />
                                ))}
                            </div>
                        ) : (
                            <EmptyState
                                icon={PackageCheck}
                                title="No extensions installed"
                                description="Install a built-in or registry extension to see it here."
                            />
                        )}
                    </section>
                </TabsContent>

                <TabsContent value="plugins">
                    <div className="plugins-section">
                        <section className="marketplace-section">
                            <SectionHeader
                                kicker="Installer"
                                title="Install ServerKit plugin"
                                meta={titleCase(installSource)}
                            />
                            <div className="plugin-install-form">
                                <div className="plugin-install-form__heading">
                                    <div className="plugin-install-form__icon">
                                        <PlugZap aria-hidden="true" />
                                    </div>
                                    <div>
                                        <h3>Plugin source</h3>
                                        <p className="text-muted">Load plugin packages from a repository, host folder, or zip archive.</p>
                                    </div>
                                </div>

                                <div className="plugin-install-tabs" role="tablist" aria-label="Plugin install source">
                                    {PLUGIN_INSTALL_SOURCES.map((source) => {
                                        const SourceIcon = source.icon;
                                        return (
                                            <button
                                                key={source.id}
                                                role="tab"
                                                type="button"
                                                aria-selected={installSource === source.id}
                                                className={`plugin-install-tab ${installSource === source.id ? 'plugin-install-tab--active' : ''}`}
                                                onClick={() => setInstallSource(source.id)}
                                            >
                                                <SourceIcon aria-hidden="true" />
                                                {source.label}
                                            </button>
                                        );
                                    })}
                                </div>

                                {installSource === 'url' && (
                                    <PluginInstallInput
                                        description="Paste a GitHub repo URL, release URL, or direct zip link."
                                        placeholder="https://github.com/user/serverkit-plugin"
                                        value={pluginUrl}
                                        onChange={setPluginUrl}
                                        onInstall={handlePluginInstall}
                                        disabled={installing}
                                        installDisabled={installing || !pluginUrl.trim()}
                                    />
                                )}

                                {installSource === 'path' && (
                                    <PluginInstallInput
                                        description="Use an absolute path that exists on the backend host or inside the backend container."
                                        placeholder="/opt/serverkit/plugins/my-plugin"
                                        value={pluginPath}
                                        onChange={setPluginPath}
                                        onInstall={handlePluginInstall}
                                        disabled={installing}
                                        installDisabled={installing || !pluginPath.trim()}
                                    />
                                )}

                                {installSource === 'upload' && (
                                    <div className="plugin-install-source">
                                        <p className="text-muted">
                                            Upload a plugin zip with <code>plugin.json</code> at the top level or one folder deep.
                                        </p>
                                        <div className="plugin-install-row">
                                            <Input
                                                type="file"
                                                className="marketplace-file-input"
                                                accept=".zip,application/zip,application/x-zip-compressed"
                                                disabled={installing}
                                                onChange={(event) => setPluginFile(event.target.files?.[0] || null)}
                                            />
                                            <Button
                                                onClick={handlePluginInstall}
                                                disabled={installing || !pluginFile}
                                            >
                                                <DownloadCloud aria-hidden="true" />
                                                {installing ? 'Installing...' : 'Install'}
                                            </Button>
                                        </div>
                                        {pluginFile && (
                                            <div className="plugin-file-note">
                                                {pluginFile.name} | {(pluginFile.size / 1024).toFixed(1)} KB
                                            </div>
                                        )}
                                    </div>
                                )}
                            </div>
                        </section>

                        <section className="marketplace-section">
                            <SectionHeader
                                kicker="Runtime"
                                title="Installed ServerKit plugins"
                                meta={`${plugins.length} plugins`}
                            />
                            {plugins.length > 0 ? (
                                <div className="installed-list">
                                    {plugins.map((plugin) => (
                                        <PluginRow
                                            key={plugin.id}
                                            plugin={plugin}
                                            update={updatesByKey.get(String(plugin.id))}
                                            installing={installing}
                                            onToggle={handlePluginToggle}
                                            onUpdate={handlePluginUpdate}
                                            onUninstall={requestPluginUninstall}
                                            onConfigure={setConfigTarget}
                                            statusVariant={pluginStatusVariant}
                                        />
                                    ))}
                                </div>
                            ) : (
                                <EmptyState
                                    icon={PlugZap}
                                    title="No ServerKit plugins installed"
                                    description="Install a plugin package to extend the panel runtime."
                                />
                            )}
                        </section>
                    </div>
                </TabsContent>
            </Tabs>

            {detailEntry && (
                <ExtensionDetailModal
                    entry={detailEntry}
                    installing={installing}
                    statusVariant={pluginStatusVariant}
                    onClose={() => setDetailEntry(null)}
                    onInstall={() => {
                        installEntry(detailEntry);
                        setDetailEntry(null);
                    }}
                />
            )}

            {uninstallTarget && (
                <PluginUninstallDialog
                    plugin={uninstallTarget}
                    onCancel={() => setUninstallTarget(null)}
                    onConfirm={confirmPluginUninstall}
                />
            )}

            {configTarget && (
                <PluginConfigDialog
                    plugin={configTarget}
                    onClose={() => setConfigTarget(null)}
                />
            )}
        </div>
    );
};

const SectionHeader = ({ kicker, title, meta }) => (
    <div className="marketplace-section__header">
        <div>
            <p className="marketplace-kicker">{kicker}</p>
            <h2>{title}</h2>
        </div>
        {meta && <Badge variant="outline">{meta}</Badge>}
    </div>
);

const CatalogExtensionCard = ({ entry, installing, onInstall, onOpenDetail, statusVariant }) => {
    const category = entry.category || 'utility';
    const Icon = getCategoryIcon(category);
    const isLocal = entry.source === 'local';
    const installedLabel = entry.status && entry.status !== 'active'
        ? titleCase(entry.status)
        : 'Installed';

    const openDetail = () => onOpenDetail(entry);
    const handleKeyDown = (event) => {
        if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            openDetail();
        }
    };

    return (
        <article
            className={`extension-card extension-card--${entry.source} extension-card--${category} extension-card--clickable card`}
            role="button"
            tabIndex={0}
            onClick={openDetail}
            onKeyDown={handleKeyDown}
        >
            <div className="extension-card__topline">
                <div className={`extension-card__icon extension-card__icon--${category}`}>
                    <Icon aria-hidden="true" />
                </div>
                <div className="extension-card__badges">
                    {entry.firstParty && (
                        <Badge variant="secondary" className="extension-firstparty">by ServerKit</Badge>
                    )}
                    <Badge variant={sourceBadgeVariant(entry.source)}>{entry.sourceLabel}</Badge>
                    <Badge variant="outline">{titleCase(category)}</Badge>
                </div>
            </div>
            <div className="extension-card__body">
                <h3>{entry.displayName}</h3>
                <p className="extension-card__desc">{entry.description}</p>
            </div>
            <div className="extension-card__signals">
                <span>{entry.sourceDetail}</span>
                <span>{entry.installed ? installedLabel : 'Ready to install'}</span>
            </div>
            <div className="extension-card__footer">
                <div className="extension-card__info">
                    <span>v{entry.version}</span>
                    {entry.author && <span>by {entry.author}</span>}
                    {entry.extensionType && (
                        <Badge variant="secondary">
                            {isLocal ? 'built-in' : entry.extensionType}
                        </Badge>
                    )}
                </div>
                <div className="extension-card__actions">
                    {entry.installed ? (
                        <Badge variant={isLocal ? statusVariant(entry.status) : 'success'}>
                            <CheckCircle2 aria-hidden="true" />
                            {installedLabel}
                        </Badge>
                    ) : (
                        <Button
                            size="sm"
                            disabled={installing}
                            onClick={(event) => {
                                event.stopPropagation();
                                onInstall(entry.installKey);
                            }}
                        >
                            <DownloadCloud aria-hidden="true" />
                            {installing ? 'Installing...' : 'Install'}
                        </Button>
                    )}
                </div>
            </div>
        </article>
    );
};

const ExtensionDetailModal = ({ entry, installing, statusVariant, onClose, onInstall }) => {
    const category = entry.category || 'utility';
    const Icon = getCategoryIcon(category);
    const isLocal = entry.source === 'local';
    const iconSvg = entry.icon ? sanitizeSvgInner(entry.icon) : '';
    const screenshots = entry.screenshots || [];
    const permissions = Array.isArray(entry.permissions) ? entry.permissions : [];
    const configKeys = entry.configSchema && typeof entry.configSchema === 'object'
        ? Object.keys(entry.configSchema)
        : [];
    const installedLabel = entry.status && entry.status !== 'active'
        ? titleCase(entry.status)
        : 'Installed';

    return (
        <Modal open onClose={onClose} title={entry.displayName} size="lg">
            <div className="extension-detail">
                <div className="extension-detail__header">
                    <div className={`extension-detail__icon extension-detail__icon--${category}`}>
                        {iconSvg ? (
                            <svg
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                aria-hidden="true"
                                focusable="false"
                                dangerouslySetInnerHTML={{ __html: iconSvg }}
                            />
                        ) : (
                            <Icon aria-hidden="true" />
                        )}
                    </div>
                    <div className="extension-detail__heading">
                        <div className="extension-detail__badges">
                            {entry.firstParty && (
                                <Badge variant="secondary" className="extension-firstparty">by ServerKit</Badge>
                            )}
                            <Badge variant={sourceBadgeVariant(entry.source)}>{entry.sourceLabel}</Badge>
                            <Badge variant="outline">{titleCase(category)}</Badge>
                        </div>
                        <div className="extension-detail__meta">
                            <span>v{entry.version}</span>
                            {entry.author && <span>by {entry.author}</span>}
                            <span>{isLocal ? 'built-in' : entry.extensionType}</span>
                        </div>
                    </div>
                </div>

                <p className="extension-detail__desc">{entry.description}</p>

                {permissions.length > 0 && (
                    <div className="extension-detail__consent">
                        <p className="extension-detail__section-label">This extension requests:</p>
                        <div className="extension-detail__chips">
                            {permissions.map((permission) => (
                                <Badge key={permission} variant="outline">{permission}</Badge>
                            ))}
                        </div>
                    </div>
                )}

                {configKeys.length > 0 && (
                    <div className="extension-detail__config">
                        <p className="extension-detail__section-label">Configuration</p>
                        <ul className="extension-detail__config-list">
                            {configKeys.map((key) => (
                                <li key={key}><code>{key}</code></li>
                            ))}
                        </ul>
                    </div>
                )}

                {screenshots.length > 0 && (
                    <div className="extension-detail__gallery" aria-label="Screenshots">
                        {screenshots.map((src, index) => (
                            <img
                                key={src}
                                src={src}
                                alt={`${entry.displayName} screenshot ${index + 1}`}
                                className="extension-detail__shot"
                                loading="lazy"
                            />
                        ))}
                    </div>
                )}

                <div className="extension-detail__actions">
                    {entry.installed ? (
                        <Badge variant={isLocal ? statusVariant(entry.status) : 'success'}>
                            <CheckCircle2 aria-hidden="true" />
                            {installedLabel}
                        </Badge>
                    ) : (
                        <Button disabled={installing} onClick={onInstall}>
                            <DownloadCloud aria-hidden="true" />
                            {installing ? 'Installing...' : 'Install'}
                        </Button>
                    )}
                </div>
            </div>
        </Modal>
    );
};

// Installed catalog entries are builtin installs (the legacy "published"
// lane was retired, #51); their manage/uninstall actions live on the
// ServerKit Plugins tab, so this row is informational.
const InstalledCatalogRow = ({ entry }) => (
    <article className="installed-item card">
        <div className="installed-item__main">
            <div className="installed-item__icon installed-item__icon--local">
                <Archive aria-hidden="true" />
            </div>
            <div className="installed-item__content">
                <div className="installed-item__title-line">
                    <strong>{entry.displayName}</strong>
                    <span className="text-muted">v{entry.version}</span>
                    <Badge variant="warning">{entry.sourceLabel}</Badge>
                </div>
            </div>
        </div>
        <div className="installed-item__actions">
            <Badge variant="success">Installed</Badge>
        </div>
    </article>
);

const PluginRow = ({ plugin, update, installing, onToggle, onUpdate, onUninstall, onConfigure, statusVariant }) => {
    const updateAvailable = Boolean(update?.update_available);
    const compatible = update?.compatible !== false;
    const configurable = plugin.config_schema
        && typeof plugin.config_schema === 'object'
        && Object.keys(plugin.config_schema).length > 0;

    return (
        <article className={`installed-item installed-item--plugin card ${plugin.status === 'error' ? 'installed-item--error' : ''}`}>
            <div className="installed-item__main">
                <div className="installed-item__icon installed-item__icon--plugin">
                    <PlugZap aria-hidden="true" />
                </div>
                <div className="installed-item__content">
                    <div className="installed-item__title-line">
                        <strong>{plugin.display_name}</strong>
                        <span className="text-muted">v{plugin.version}</span>
                        <Badge variant={statusVariant(plugin.status)}>{plugin.status}</Badge>
                        {plugin.has_backend && <Badge variant="secondary">Backend</Badge>}
                        {plugin.has_frontend && <Badge variant="secondary">Frontend</Badge>}
                        {updateAvailable && (
                            <Badge variant="info" className="plugin-update-badge">
                                Update available → v{update.available_version}
                            </Badge>
                        )}
                    </div>
                    {plugin.description && <p className="installed-item__description">{plugin.description}</p>}
                    {plugin.error_message && <p className="installed-item__error">{plugin.error_message}</p>}
                </div>
            </div>
            <div className="installed-item__actions">
                {updateAvailable && (
                    <Button
                        size="sm"
                        disabled={!compatible || installing}
                        title={compatible ? undefined : 'Panel version is too old for this update'}
                        onClick={() => onUpdate(plugin.id)}
                    >
                        <DownloadCloud aria-hidden="true" />
                        Update
                    </Button>
                )}
                {configurable && (
                    <Button size="sm" variant="outline" onClick={() => onConfigure(plugin)}>
                        Configure
                    </Button>
                )}
                <Button
                    size="sm"
                    variant={plugin.status === 'active' ? 'outline' : 'default'}
                    onClick={() => onToggle(plugin)}
                >
                    {plugin.status === 'active' ? 'Disable' : 'Enable'}
                </Button>
                <Button size="sm" variant="destructive" onClick={() => onUninstall(plugin)}>
                    Uninstall
                </Button>
            </div>
        </article>
    );
};

// Config editor for an installed plugin (#49). Fields come from the manifest's
// config_schema (top-level keys, or JSON-schema `properties`); values persist
// via PUT /plugins/<id>/config and the plugin reads them on the backend via
// plugins_sdk.config(slug).
const PluginConfigDialog = ({ plugin, onClose }) => {
    const toast = useToast();
    const [values, setValues] = useState(null);
    const [saving, setSaving] = useState(false);

    const schema = plugin.config_schema || {};
    const fields = schema.properties && typeof schema.properties === 'object'
        ? schema.properties
        : schema;

    useEffect(() => {
        api.getPluginConfig(plugin.id)
            .then((data) => setValues(data.config || {}))
            .catch(() => setValues({}));
    }, [plugin.id]);

    const setField = (key, v) => setValues((prev) => ({ ...prev, [key]: v }));

    const save = async () => {
        setSaving(true);
        try {
            await api.updatePluginConfig(plugin.id, values || {});
            toast.success('Plugin configuration saved');
            onClose();
        } catch (err) {
            toast.error(err.message || 'Failed to save configuration');
        } finally {
            setSaving(false);
        }
    };

    return (
        <Modal
            open
            onClose={onClose}
            title={`Configure ${plugin.display_name}`}
            size="sm"
            footer={
                <>
                    <Button variant="ghost" onClick={onClose}>Cancel</Button>
                    <Button onClick={save} disabled={saving || values === null}>
                        {saving ? 'Saving…' : 'Save'}
                    </Button>
                </>
            }
        >
            {values === null ? (
                <p className="text-muted">Loading…</p>
            ) : (
                <div className="plugin-config-form">
                    {Object.entries(fields).map(([key, spec]) => {
                        const s = spec && typeof spec === 'object' ? spec : {};
                        const type = s.type || 'string';
                        const isNumber = type === 'number' || type === 'integer';
                        const value = values[key] ?? s.default ?? (type === 'boolean' ? false : '');
                        return (
                            <label key={key} className="plugin-config-form__field">
                                <span className="plugin-config-form__label">{s.title || key}</span>
                                {type === 'boolean' ? (
                                    <input
                                        type="checkbox"
                                        checked={Boolean(value)}
                                        onChange={(e) => setField(key, e.target.checked)}
                                    />
                                ) : Array.isArray(s.enum) ? (
                                    <select
                                        className="ui-input"
                                        value={value}
                                        onChange={(e) => setField(key, e.target.value)}
                                    >
                                        {s.enum.map((opt) => (
                                            <option key={opt} value={opt}>{opt}</option>
                                        ))}
                                    </select>
                                ) : (
                                    <input
                                        className="ui-input"
                                        type={isNumber ? 'number' : (s.secret ? 'password' : 'text')}
                                        value={value}
                                        onChange={(e) => setField(
                                            key,
                                            isNumber
                                                ? (e.target.value === '' ? '' : Number(e.target.value))
                                                : e.target.value
                                        )}
                                    />
                                )}
                                {s.description && (
                                    <span className="plugin-config-form__hint text-muted">{s.description}</span>
                                )}
                            </label>
                        );
                    })}
                    {Object.keys(fields).length === 0 && (
                        <p className="text-muted">This plugin declares no configuration fields.</p>
                    )}
                </div>
            )}
        </Modal>
    );
};

// Data-policy dialog for plugin uninstall. Keeping data (default) leaves the
// extension's tables intact for a later reinstall; purging drops them.
const PluginUninstallDialog = ({ plugin, onCancel, onConfirm }) => (
    <Modal
        open
        onClose={onCancel}
        title={`Uninstall ${plugin.display_name}?`}
        size="sm"
        footer={
            <>
                <Button variant="ghost" onClick={onCancel}>Cancel</Button>
                <Button variant="outline" onClick={() => onConfirm(false)}>Keep data</Button>
                <Button variant="destructive" onClick={() => onConfirm(true)}>Purge data</Button>
            </>
        }
    >
        <div className="plugin-uninstall-dialog">
            <p>Removing this extension stops its routes and UI contributions.</p>
            <p className="text-muted">
                <strong>Keep data</strong> leaves the extension&apos;s database tables intact so you can
                reinstall later. <strong>Purge data</strong> permanently drops the extension&apos;s tables
                and cannot be undone.
            </p>
        </div>
    </Modal>
);

const PluginInstallInput = ({
    description,
    placeholder,
    value,
    onChange,
    onInstall,
    disabled,
    installDisabled,
}) => (
    <div className="plugin-install-source">
        <p className="text-muted">{description}</p>
        <div className="plugin-install-row">
            <Input
                placeholder={placeholder}
                value={value}
                onChange={(event) => onChange(event.target.value)}
                onKeyDown={(event) => event.key === 'Enter' && onInstall()}
                disabled={disabled}
            />
            <Button onClick={onInstall} disabled={installDisabled}>
                <DownloadCloud aria-hidden="true" />
                {disabled ? 'Installing...' : 'Install'}
            </Button>
        </div>
    </div>
);

const RuntimeRow = ({ label, value, danger }) => (
    <div className={`marketplace-runtime__row ${danger ? 'marketplace-runtime__row--danger' : ''}`}>
        <span>{label}</span>
        <strong>{value}</strong>
    </div>
);

export default Marketplace;
