import { useState, useEffect, useMemo, useCallback, useRef } from 'react';
import {
    PanelLeftClose, PanelLeftOpen, Search, X, RefreshCw, Plus, Terminal,
    Archive, Database, Table2, Server, ChevronDown,
    Trash2, DatabaseBackup, Copy, FileCode2, Lock, BookMarked,
} from 'lucide-react';
import api from '../services/api';
import Modal from '@/components/Modal';
import ManagedDatabasesPanel from '../components/databases/ManagedDatabasesPanel';
import { formatBytes } from '@/utils/formatBytes';
import { useToast } from '../contexts/ToastContext';
import { useConfirm } from '../hooks/useConfirm';
import EmptyState from '../components/EmptyState';
import SourceTree from '../components/databases/SourceTree';
import ConsoleTab from '../components/databases/ConsoleTab';
import TableDataTab from '../components/databases/TableDataTab';
import BackupsTab from '../components/databases/BackupsTab';
import {
    CreateMySQLDatabaseModal, CreateMySQLUserModal,
    CreatePostgreSQLDatabaseModal, CreatePostgreSQLUserModal,
} from '../components/databases/modals';
import { listTables, connKey, connLabel, quoteIdent, ENGINE_META } from '../components/databases/dbAdapter';

const SIDEBAR_KEY = 'serverkit-dbx-sidebar';

function engineState(engine, status) {
    if (engine !== 'mysql' && engine !== 'postgresql') return 'available';
    const s = status?.[engine];
    if (!s) return 'available';
    if (!s.installed) return 'missing';
    return s.running ? 'active' : 'inactive';
}

// ─── node builders ───────────────────────────────────────────
function dbNode(engine, conn, label, size, idOverride) {
    return {
        id: idOverride || `${engine}:db:${label}`,
        kind: 'database', engine, label, expandable: true, conn,
        sizeText: size ? formatBytes(size) : null,
    };
}

// A database living inside a Docker container, surfaced under its engine node
// (e.g. a WordPress stack's MySQL appears under "MySQL / MariaDB"). Tagged with
// source/appName so the tree can show a Docker badge.
function dockerDbNode(engine, db, i) {
    return {
        id: `${engine}:docker:${db.container}:${db.database || 'default'}:${i}`,
        kind: 'database', engine, label: db.database || 'default', expandable: true,
        conn: {
            dbType: 'docker', container: db.container, name: db.database,
            password: db.password || db.root_password, user: db.user, dockerType: db.type,
        },
        source: 'docker', appName: db.app_name,
    };
}

function TabIcon({ tab }) {
    if (tab.kind === 'backups') return <Archive size={13} aria-hidden="true" />;
    if (tab.kind === 'console') return <Terminal size={13} aria-hidden="true" />;
    return <Table2 size={13} aria-hidden="true" />;
}

export default function Databases() {
    const toast = useToast();
    const { confirm } = useConfirm();

    const [status, setStatus] = useState(null);
    const [statusLoading, setStatusLoading] = useState(true);
    const [isAdmin, setIsAdmin] = useState(false);

    const [expanded, setExpanded] = useState(new Set());
    const [childrenCache, setChildrenCache] = useState(new Map());
    const [loadingNodes, setLoadingNodes] = useState(new Set());
    const [selectedNode, setSelectedNode] = useState(null);

    const [tabs, setTabs] = useState([]);
    const [activeTabId, setActiveTabId] = useState(null);
    const [tabStatuses, setTabStatuses] = useState({});

    const [sidebarVisible, setSidebarVisible] = useState(() => localStorage.getItem(SIDEBAR_KEY) !== 'false');
    const [filter, setFilter] = useState('');
    const [ctxMenu, setCtxMenu] = useState(null);
    const [showNewMenu, setShowNewMenu] = useState(false);
    const [showManaged, setShowManaged] = useState(false);
    const [modal, setModal] = useState(null); // { type, databases }
    const newMenuRef = useRef(null);
    const didAutoExpand = useRef(false);

    useEffect(() => { localStorage.setItem(SIDEBAR_KEY, String(sidebarVisible)); }, [sidebarVisible]);

    useEffect(() => {
        (async () => {
            try {
                const data = await api.getDatabaseStatus();
                setStatus(data);
            } catch (err) {
                console.error('Failed to get database status:', err);
            } finally {
                setStatusLoading(false);
            }
            try {
                const user = await api.getCurrentUser();
                setIsAdmin(user.role === 'admin');
            } catch { /* non-admin / not logged in handled by route guard */ }
        })();
    }, []);

    const roots = useMemo(() => ([
        // mysql/postgresql stay expandable even when the host engine is absent —
        // they can still contain databases that live in Docker containers.
        { id: 'eng:mysql', kind: 'engine', engine: 'mysql', label: ENGINE_META.mysql.label, status: engineState('mysql', status), expandable: true },
        { id: 'eng:postgresql', kind: 'engine', engine: 'postgresql', label: ENGINE_META.postgresql.label, status: engineState('postgresql', status), expandable: true },
        { id: 'eng:sqlite', kind: 'engine', engine: 'sqlite', label: ENGINE_META.sqlite.label, status: 'available', expandable: true },
        { id: 'eng:docker', kind: 'engine', engine: 'docker', label: ENGINE_META.docker.label, status: 'available', expandable: true },
    ]), [status]);

    // ─── lazy child loading ───────────────────────────────────
    const loadChildren = useCallback(async (node) => {
        if (node.kind === 'engine') {
            if (node.engine === 'mysql') {
                const [host, docker] = await Promise.all([
                    api.getMySQLDatabases().catch(() => ({ databases: [] })),
                    api.getAllDockerDatabases().catch(() => ({ databases: [] })),
                ]);
                const hostNodes = (host.databases || []).map((db) => dbNode('mysql', { dbType: 'mysql', name: db.name }, db.name, db.size));
                const dockerNodes = (docker.databases || []).filter((db) => db.type === 'mysql').map((db, i) => dockerDbNode('mysql', db, i));
                return [...hostNodes, ...dockerNodes];
            }
            if (node.engine === 'postgresql') {
                const [host, docker] = await Promise.all([
                    api.getPostgreSQLDatabases().catch(() => ({ databases: [] })),
                    api.getAllDockerDatabases().catch(() => ({ databases: [] })),
                ]);
                const hostNodes = (host.databases || []).map((db) => dbNode('postgresql', { dbType: 'postgresql', name: db.name }, db.name, db.size));
                const dockerNodes = (docker.databases || []).filter((db) => db.type === 'postgresql').map((db, i) => dockerDbNode('postgresql', db, i));
                return [...hostNodes, ...dockerNodes];
            }
            if (node.engine === 'sqlite') {
                const d = await api.getSQLiteDatabases();
                return (d.databases || []).map((db) => dbNode('sqlite', { dbType: 'sqlite', name: db.name, path: db.path }, db.name, db.size, `sqlite:db:${db.path}`));
            }
            if (node.engine === 'docker') {
                const d = await api.getApps();
                return (d.apps || []).filter((a) => a.app_type === 'docker').map((app) => ({
                    id: `app:${app.id}`, kind: 'app', engine: 'docker', label: app.name, expandable: true, appId: app.id,
                }));
            }
        }
        if (node.kind === 'app') {
            const d = await api.getAppDatabases(node.appId);
            return (d.databases || []).map((db, i) => ({
                // engine = the brand (mysql/postgresql) so the row shows the right
                // brand icon/tint; the connection is still routed over docker exec.
                id: `app:${node.appId}:db:${i}`, kind: 'database', engine: db.type || 'docker',
                label: db.database || 'default', expandable: true,
                conn: { dbType: 'docker', container: db.container, name: db.database, password: db.password || db.root_password, user: db.user, dockerType: db.type },
            }));
        }
        if (node.kind === 'database') {
            const d = await listTables(node.conn);
            // A docker database reports `connected: false` when the container
            // exec/auth fails — surface that as an error row instead of letting
            // it masquerade as an empty database.
            if (d && d.connected === false) {
                const e = new Error(d.error || 'connection failed');
                e.userMessage = d.error
                    ? `Couldn't connect: ${d.error}`
                    : `Couldn't connect to ${connLabel(node.conn)}. Is the container running?`;
                throw e;
            }
            return (d.tables || []).map((t) => ({
                id: `${node.id}:t:${t.name}`, kind: 'table', engine: node.engine, label: t.name,
                expandable: false, conn: node.conn, table: t.name,
                rows: typeof t.rows === 'number' ? t.rows : null,
            }));
        }
        return [];
    }, []);

    const fetchChildren = useCallback(async (node) => {
        setLoadingNodes((s) => new Set(s).add(node.id));
        try {
            const kids = await loadChildren(node);
            setChildrenCache((c) => new Map(c).set(node.id, kids));
        } catch (err) {
            console.error('Failed to load tree node:', err);
            setChildrenCache((c) => new Map(c).set(node.id, { __error: err.userMessage || "Couldn't load. Right-click to retry." }));
        } finally {
            setLoadingNodes((s) => { const n = new Set(s); n.delete(node.id); return n; });
        }
    }, [loadChildren]);

    const toggle = useCallback((node) => {
        const willOpen = !expanded.has(node.id);
        setExpanded((prev) => { const n = new Set(prev); if (willOpen) n.add(node.id); else n.delete(node.id); return n; });
        if (willOpen && !childrenCache.has(node.id)) fetchChildren(node);
    }, [expanded, childrenCache, fetchChildren]);

    const refresh = useCallback((node) => {
        setChildrenCache((c) => { const n = new Map(c); n.delete(node.id); return n; });
        if (expanded.has(node.id)) fetchChildren(node);
    }, [expanded, fetchChildren]);

    // Auto-expand the first running engine so the tree isn't empty on arrival.
    useEffect(() => {
        if (statusLoading || didAutoExpand.current) return;
        const first = roots.find((r) => r.status === 'active');
        if (first) {
            didAutoExpand.current = true;
            setExpanded((prev) => new Set(prev).add(first.id));
            fetchChildren(first);
        }
    }, [statusLoading, roots, fetchChildren]);

    // ─── tabs ─────────────────────────────────────────────────
    const reportStatus = useCallback((tabId, s) => {
        setTabStatuses((prev) => ({ ...prev, [tabId]: s }));
    }, []);

    function openTableTab(node) {
        const id = `tbl:${connKey(node.conn)}:${node.table}`;
        setTabs((prev) => prev.some((t) => t.id === id) ? prev
            : [...prev, { id, kind: 'table', title: node.table, conn: node.conn, table: node.table, rows: node.rows, engine: node.engine }]);
        setActiveTabId(id);
    }

    function openConsole(conn, engine, initialQuery = '') {
        const id = `con:${connKey(conn)}`;
        setTabs((prev) => prev.some((t) => t.id === id) ? prev
            : [...prev, { id, kind: 'console', title: `${connLabel(conn)}`, conn, engine, initialQuery }]);
        setActiveTabId(id);
    }

    function openBackups() {
        setTabs((prev) => prev.some((t) => t.id === 'backups') ? prev : [...prev, { id: 'backups', kind: 'backups', title: 'Backups' }]);
        setActiveTabId('backups');
    }

    function closeTab(id, e) {
        e?.stopPropagation();
        setTabs((prev) => {
            const idx = prev.findIndex((t) => t.id === id);
            const next = prev.filter((t) => t.id !== id);
            setActiveTabId((cur) => {
                if (cur !== id) return cur;
                const fallback = next[idx] || next[idx - 1];
                return fallback ? fallback.id : null;
            });
            return next;
        });
        setTabStatuses((prev) => { const n = { ...prev }; delete n[id]; return n; });
    }

    function activate(node) {
        setSelectedNode(node);
        if (node.kind === 'table') openTableTab(node);
        else if (node.kind === 'database') {
            // single click opens the database's SQL console (it used to be
            // reachable only via the context menu) and expands its tables;
            // collapsing stays on the chevron so re-clicks don't fold the tree
            openConsole(node.conn, node.engine);
            if (node.expandable && !expanded.has(node.id)) toggle(node);
        } else if (node.expandable) toggle(node);
    }

    // ─── tree context menu ────────────────────────────────────
    function openContext(e, node) {
        e.preventDefault();
        e.stopPropagation();
        setSelectedNode(node);
        const menuW = 220;
        const x = Math.min(e.clientX, window.innerWidth - menuW - 8);
        setCtxMenu({ x, y: e.clientY, node });
    }

    useEffect(() => {
        if (!ctxMenu && !showNewMenu) return;
        const close = (e) => {
            if (showNewMenu && newMenuRef.current?.contains(e.target)) return;
            setCtxMenu(null);
            setShowNewMenu(false);
        };
        const onEsc = (e) => { if (e.key === 'Escape') { setCtxMenu(null); setShowNewMenu(false); } };
        document.addEventListener('click', close);
        document.addEventListener('keydown', onEsc);
        return () => { document.removeEventListener('click', close); document.removeEventListener('keydown', onEsc); };
    }, [ctxMenu, showNewMenu]);

    async function backupDatabase(node) {
        try {
            const res = node.engine === 'mysql' ? await api.backupMySQLDatabase(node.label) : await api.backupPostgreSQLDatabase(node.label);
            if (res.success) toast.success(`Backup created: ${res.backup_path}`);
        } catch {
            toast.error('Failed to create backup');
        }
    }

    async function dropDatabase(node) {
        const ok = await confirm({
            title: 'Drop database',
            message: `Drop database "${node.label}"? This permanently deletes the database and all its data.`,
            confirmText: `Drop ${node.label}`,
            variant: 'danger',
        });
        if (!ok) return;
        try {
            if (node.engine === 'mysql') await api.dropMySQLDatabase(node.label);
            else await api.dropPostgreSQLDatabase(node.label);
            toast.success(`Dropped database "${node.label}"`);
            const eng = roots.find((r) => r.engine === node.engine);
            if (eng) refresh(eng);
            setTabs((prev) => prev.filter((t) => !(t.conn && connKey(t.conn) === connKey(node.conn))));
        } catch {
            toast.error('Failed to drop database');
        }
    }

    function copyName(node) {
        navigator.clipboard?.writeText(node.label).then(
            () => toast.success('Copied name'),
            () => toast.error('Could not copy'),
        );
    }

    function ctxActions(node) {
        switch (node.kind) {
            case 'engine':
                if (node.engine === 'mysql' && node.status === 'active') {
                    return [
                        { label: 'Create database', icon: Plus, onClick: () => setModal({ type: 'mysql-db' }) },
                        { label: 'Create user', icon: Plus, onClick: () => openUserModal('mysql') },
                        { label: 'Refresh', icon: RefreshCw, onClick: () => refresh(node) },
                    ];
                }
                if (node.engine === 'postgresql' && node.status === 'active') {
                    return [
                        { label: 'Create database', icon: Plus, onClick: () => setModal({ type: 'pg-db' }) },
                        { label: 'Create user', icon: Plus, onClick: () => openUserModal('postgresql') },
                        { label: 'Refresh', icon: RefreshCw, onClick: () => refresh(node) },
                    ];
                }
                return [{ label: 'Refresh', icon: RefreshCw, onClick: () => refresh(node) }];
            case 'database': {
                const actions = [
                    { label: 'Open SQL console', icon: Terminal, onClick: () => openConsole(node.conn, node.engine) },
                    { label: 'Refresh tables', icon: RefreshCw, onClick: () => refresh(node) },
                ];
                if (node.engine === 'mysql' || node.engine === 'postgresql') {
                    actions.splice(1, 0, { label: 'Back up database', icon: DatabaseBackup, onClick: () => backupDatabase(node) });
                    actions.push({ label: 'Drop database', icon: Trash2, danger: true, onClick: () => dropDatabase(node) });
                }
                return actions;
            }
            case 'app':
                return [{ label: 'Refresh', icon: RefreshCw, onClick: () => refresh(node) }];
            case 'table':
                return [
                    { label: 'Open data', icon: Table2, onClick: () => openTableTab(node) },
                    { label: 'Query in console', icon: FileCode2, onClick: () => openConsole(node.conn, node.engine, `SELECT * FROM ${quoteIdent(node.conn, node.table)} LIMIT 100;`) },
                    { label: 'Copy name', icon: Copy, onClick: () => copyName(node) },
                ];
            default:
                return [];
        }
    }

    async function openUserModal(engine) {
        try {
            const d = engine === 'mysql' ? await api.getMySQLDatabases() : await api.getPostgreSQLDatabases();
            setModal({ type: engine === 'mysql' ? 'mysql-user' : 'pg-user', databases: d.databases || [] });
        } catch {
            setModal({ type: engine === 'mysql' ? 'mysql-user' : 'pg-user', databases: [] });
        }
    }

    function onModalCreated() {
        // Refresh the affected engine's tree so new databases/users appear.
        const engine = modal?.type?.startsWith('mysql') ? 'mysql' : 'postgresql';
        const eng = roots.find((r) => r.engine === engine);
        if (eng) refresh(eng);
    }

    const newConsoleConn = selectedNode?.conn || null;
    const activeStatus = tabStatuses[activeTabId];

    const treeHandlers = useMemo(() => ({
        onToggle: toggle,
        onActivate: activate,
        onContext: openContext,
    }), [toggle]); // eslint-disable-line react-hooks/exhaustive-deps

    return (
        <div className="page-container page-container--full-bleed db-explorer">
            {/* ─── Toolbar ─────────────────────────────── */}
            <header className="dbx-toolbar">
                <div className="dbx-toolbar-left">
                    <button
                        type="button"
                        className="dbx-icon-btn"
                        onClick={() => setSidebarVisible((v) => !v)}
                        aria-label={sidebarVisible ? 'Hide sources' : 'Show sources'}
                        title={sidebarVisible ? 'Hide sources' : 'Show sources'}
                    >
                        {sidebarVisible ? <PanelLeftClose size={16} aria-hidden="true" /> : <PanelLeftOpen size={16} aria-hidden="true" />}
                    </button>
                    <h1 className="dbx-title"><Database size={17} aria-hidden="true" /> Database Explorer</h1>
                </div>

                <div className="dbx-toolbar-right">
                    <div className="dbx-new" ref={newMenuRef}>
                        <button
                            type="button"
                            className="dbx-primary"
                            onClick={() => setShowNewMenu((s) => !s)}
                            aria-haspopup="menu"
                            aria-expanded={showNewMenu}
                        >
                            <Plus size={15} aria-hidden="true" /> New <ChevronDown size={13} aria-hidden="true" />
                        </button>
                        {showNewMenu && (
                            <div className="dbx-menu" role="menu">
                                <button
                                    type="button"
                                    role="menuitem"
                                    disabled={!newConsoleConn}
                                    onClick={() => { if (newConsoleConn) openConsole(newConsoleConn, selectedNode.engine); setShowNewMenu(false); }}
                                >
                                    <Terminal size={14} aria-hidden="true" /> SQL console
                                    {!newConsoleConn && <span className="dbx-menu-hint">select a database</span>}
                                </button>
                                <div className="dbx-menu-sep" />
                                <button type="button" role="menuitem" disabled={engineState('mysql', status) !== 'active'} onClick={() => { setModal({ type: 'mysql-db' }); setShowNewMenu(false); }}>
                                    <Database size={14} aria-hidden="true" /> MySQL database
                                </button>
                                <button type="button" role="menuitem" disabled={engineState('postgresql', status) !== 'active'} onClick={() => { setModal({ type: 'pg-db' }); setShowNewMenu(false); }}>
                                    <Database size={14} aria-hidden="true" /> PostgreSQL database
                                </button>
                                <div className="dbx-menu-sep" />
                                <button type="button" role="menuitem" disabled={engineState('mysql', status) !== 'active'} onClick={() => { openUserModal('mysql'); setShowNewMenu(false); }}>
                                    <Server size={14} aria-hidden="true" /> MySQL user
                                </button>
                                <button type="button" role="menuitem" disabled={engineState('postgresql', status) !== 'active'} onClick={() => { openUserModal('postgresql'); setShowNewMenu(false); }}>
                                    <Server size={14} aria-hidden="true" /> PostgreSQL user
                                </button>
                            </div>
                        )}
                    </div>
                    <button type="button" className="dbx-chip" onClick={() => setShowManaged(true)}>
                        <BookMarked size={14} aria-hidden="true" /> Managed
                    </button>
                    <button type="button" className="dbx-chip" onClick={openBackups}>
                        <Archive size={14} aria-hidden="true" /> Backups
                    </button>
                </div>
            </header>

            {/* ─── Body: tree + workspace ─────────────────── */}
            <div className={`dbx-body ${sidebarVisible ? '' : 'is-collapsed'}`}>
                {sidebarVisible && (
                    <aside className="dbx-tree-panel" aria-label="Database sources">
                        <div className="dbx-tree-search">
                            <Search size={14} aria-hidden="true" />
                            <input
                                type="text"
                                placeholder="Filter tables…"
                                value={filter}
                                onChange={(e) => setFilter(e.target.value)}
                                aria-label="Filter sources"
                            />
                            {filter && (
                                <button type="button" onClick={() => setFilter('')} aria-label="Clear filter"><X size={13} aria-hidden="true" /></button>
                            )}
                        </div>
                        <div className="dbx-tree-scroll">
                            {statusLoading ? (
                                <div className="dbx-tree-loading"><RefreshCw size={14} className="dbx-spin" aria-hidden="true" /> Checking servers…</div>
                            ) : (
                                <SourceTree
                                    roots={roots}
                                    expanded={expanded}
                                    childrenCache={childrenCache}
                                    loading={loadingNodes}
                                    activeKey={null}
                                    selectedId={selectedNode?.id}
                                    filter={filter}
                                    handlers={treeHandlers}
                                />
                            )}
                        </div>
                    </aside>
                )}

                <main className="dbx-workspace">
                    {tabs.length > 0 && (
                        <div className="dbx-tabbar" role="tablist" aria-label="Open tabs">
                            {tabs.map((tab) => (
                                <div
                                    key={tab.id}
                                    role="tab"
                                    aria-selected={tab.id === activeTabId}
                                    tabIndex={0}
                                    className={`dbx-tab is-${tab.engine || tab.kind} ${tab.id === activeTabId ? 'is-active' : ''}`}
                                    onClick={() => setActiveTabId(tab.id)}
                                    onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setActiveTabId(tab.id); } }}
                                >
                                    <TabIcon tab={tab} />
                                    <span className="dbx-tab-title">{tab.title}</span>
                                    <button
                                        type="button"
                                        className="dbx-tab-close"
                                        onClick={(e) => closeTab(tab.id, e)}
                                        aria-label={`Close ${tab.title}`}
                                    >
                                        <X size={13} aria-hidden="true" />
                                    </button>
                                </div>
                            ))}
                            <button
                                type="button"
                                className="dbx-tab dbx-tab-new"
                                disabled={!newConsoleConn}
                                onClick={() => { if (newConsoleConn) openConsole(newConsoleConn, selectedNode.engine); }}
                                title={newConsoleConn ? 'New SQL console' : 'Select a database to open a console'}
                                aria-label="New SQL console"
                            >
                                <Plus size={14} aria-hidden="true" />
                            </button>
                        </div>
                    )}

                    <div className="dbx-panes">
                        {tabs.length === 0 ? (
                            <div className="dbx-welcome">
                                <EmptyState
                                    icon={Database}
                                    title="Open a table or console"
                                    description={statusLoading
                                        ? 'Checking your database servers…'
                                        : 'Pick a table from the left to browse its rows, or open a SQL console on any database. Right-click a node for more actions.'}
                                />
                            </div>
                        ) : (
                            tabs.map((tab) => (
                                <div key={tab.id} className="dbx-tabpane" hidden={tab.id !== activeTabId}>
                                    {tab.kind === 'console' && (
                                        <ConsoleTab
                                            conn={tab.conn}
                                            tabId={tab.id}
                                            active={tab.id === activeTabId}
                                            isAdmin={isAdmin}
                                            initialQuery={tab.initialQuery}
                                            onStatus={reportStatus}
                                        />
                                    )}
                                    {tab.kind === 'table' && (
                                        <TableDataTab
                                            conn={tab.conn}
                                            tabId={tab.id}
                                            table={tab.table}
                                            rowsEstimate={tab.rows}
                                            active={tab.id === activeTabId}
                                            onStatus={reportStatus}
                                            onOpenConsole={(q) => openConsole(tab.conn, tab.engine, q)}
                                        />
                                    )}
                                    {tab.kind === 'backups' && <BackupsTab />}
                                </div>
                            ))
                        )}
                    </div>
                </main>
            </div>

            {/* ─── Status bar ─────────────────────────────── */}
            <footer className="dbx-statusbar">
                <div className="dbx-statusbar-left">
                    {activeStatus ? (
                        <>
                            <span className="dbx-status-item"><Database size={12} aria-hidden="true" /> {activeStatus.connText}</span>
                            {activeStatus.readonly != null && (
                                <span className={`dbx-status-item ${activeStatus.readonly ? '' : 'is-write'}`}>
                                    {activeStatus.readonly ? <><Lock size={11} aria-hidden="true" /> Read-only</> : 'Writes enabled'}
                                </span>
                            )}
                            <span className="dbx-status-item dbx-status-muted">UTF-8</span>
                        </>
                    ) : (
                        <span className="dbx-status-item dbx-status-muted">No tab open</span>
                    )}
                </div>
                <div className="dbx-statusbar-right">
                    {activeStatus?.rangeText && <span className="dbx-status-item">{activeStatus.rangeText}</span>}
                    {activeStatus?.rowCount != null && (
                        <span className="dbx-status-item">{activeStatus.rowCount} row{activeStatus.rowCount === 1 ? '' : 's'}{activeStatus.truncated ? ` of ${activeStatus.totalRows}` : ''}</span>
                    )}
                    {activeStatus?.execTime != null && <span className="dbx-status-item dbx-mono">{activeStatus.execTime}s</span>}
                    {activeStatus && <span className="dbx-status-item is-connected">Connected</span>}
                </div>
            </footer>

            {/* ─── Context menu ───────────────────────────── */}
            {ctxMenu && (
                <div className="dbx-context" style={{ left: ctxMenu.x, top: ctxMenu.y }} role="menu">
                    {ctxActions(ctxMenu.node).map((a) => (
                        <button
                            key={a.label}
                            type="button"
                            role="menuitem"
                            className={a.danger ? 'is-danger' : ''}
                            onClick={() => { a.onClick(); setCtxMenu(null); }}
                        >
                            <a.icon size={14} aria-hidden="true" /> {a.label}
                        </button>
                    ))}
                </div>
            )}

            {/* ─── Modals ─────────────────────────────────── */}
            {modal?.type === 'mysql-db' && <CreateMySQLDatabaseModal onClose={() => setModal(null)} onCreated={onModalCreated} />}
            {modal?.type === 'pg-db' && <CreatePostgreSQLDatabaseModal onClose={() => setModal(null)} onCreated={onModalCreated} />}
            {modal?.type === 'mysql-user' && <CreateMySQLUserModal databases={modal.databases} onClose={() => setModal(null)} onCreated={onModalCreated} />}
            {modal?.type === 'pg-user' && <CreatePostgreSQLUserModal databases={modal.databases} onClose={() => setModal(null)} onCreated={onModalCreated} />}

            <Modal open={showManaged} onClose={() => setShowManaged(false)} title="Managed databases" size="lg">
                <ManagedDatabasesPanel />
            </Modal>

        </div>
    );
}
