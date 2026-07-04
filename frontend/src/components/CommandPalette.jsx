import { useState, useEffect, useCallback, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import api from '../services/api';
import {
    CommandDialog,
    CommandInput,
    CommandList,
    CommandEmpty,
    CommandGroup,
    CommandItem,
} from '@/components/ui/command';
import { useContributions } from '../plugins/contributions';

const STATIC_PAGES = [
    { label: 'Services', path: '/services', category: 'Pages', keywords: 'apps containers' },
    { label: 'Docker', path: '/docker', category: 'Pages', keywords: 'containers images' },
    { label: 'Databases', path: '/databases', category: 'Pages', keywords: 'mysql postgres sql' },
    { label: 'Domains', path: '/domains', category: 'Pages', keywords: 'dns nginx records nameserver zones' },
    { label: 'SSL Certificates', path: '/ssl', category: 'Pages', keywords: 'https tls' },
    { label: 'Templates', path: '/templates', category: 'Pages', keywords: 'deploy one-click' },
    { label: 'Deployments', path: '/deployments', category: 'Pages', keywords: 'deploy jobs status logs' },
    // WordPress palette entries are contributed by the serverkit-wordpress extension.
    { label: 'Files', path: '/files', category: 'Pages', keywords: 'file manager explorer' },
    // The FTP Server palette entry is contributed by the serverkit-ftp extension.
    { label: 'Observability', path: '/monitoring', category: 'Pages', keywords: 'metrics uptime monitoring alerts' },
    { label: 'Backups', path: '/backups', category: 'Pages', keywords: 'snapshots restore' },
    { label: 'Cron Jobs', path: '/cron', category: 'Pages', keywords: 'schedule tasks' },
    { label: 'Security', path: '/security', category: 'Pages', keywords: 'firewall fail2ban' },
    { label: 'Terminal', path: '/terminal', category: 'Pages', keywords: 'shell ssh console' },
    { label: 'Servers', path: '/servers', category: 'Pages', keywords: 'fleet agents' },
    { label: 'Fleet Monitor', path: '/fleet-monitor', category: 'Pages', keywords: 'agents status' },
    // Status Pages / Cloud Provision / Remote Access palette entries are
    // contributed by the serverkit-status / serverkit-cloud-provision /
    // serverkit-remote-access extensions.
    { label: 'Marketplace', path: '/marketplace', category: 'Pages', keywords: 'extensions plugins' },
    { label: 'Downloads', path: '/downloads', category: 'Pages', keywords: 'agent installer' },
    { label: 'Projects', path: '/projects', category: 'Pages', keywords: 'organization group' },
    { label: 'Shared Variables', path: '/shared-variables', category: 'Pages', keywords: 'env secrets shared' },
    { label: 'Workspaces', path: '/workspaces', category: 'Pages', keywords: 'organization team' },
    // GPU Monitor + Workflow Builder command-palette entries are contributed by
    // the serverkit-gpu / serverkit-workflows builtin extensions.
    { label: 'Queue', path: '/queue', category: 'Pages', keywords: 'bus operations tasks' },
    { label: 'Jobs', path: '/jobs', category: 'Pages', keywords: 'scheduler background work' },
    { label: 'Events', path: '/telemetry', category: 'Pages', keywords: 'telemetry metrics observability system events' },
    { label: 'Vaults', path: '/vaults', category: 'Pages', keywords: 'secrets tokens credentials vault' },
    { label: 'Webhooks', path: '/webhooks', category: 'Pages', keywords: 'webhook receive forward delivery' },
    { label: 'Settings', path: '/settings', category: 'Settings', keywords: 'profile preferences' },
    { label: 'Settings: Users', path: '/settings/users', category: 'Settings', keywords: 'accounts team' },
    { label: 'Settings: API Keys', path: '/settings/api', category: 'Settings', keywords: 'tokens access' },
    { label: 'Settings: SSO', path: '/settings/sso', category: 'Settings', keywords: 'oauth saml login' },
    { label: 'Settings: Appearance', path: '/settings/appearance', category: 'Settings', keywords: 'theme dark light' },
    { label: 'Settings: Notifications', path: '/settings/notifications', category: 'Settings', keywords: 'alerts email slack' },
    { label: 'Settings: System', path: '/settings/system', category: 'Settings', keywords: 'server config' },
];

function fuzzyMatch(text, query) {
    const lower = text.toLowerCase();
    const q = query.toLowerCase();
    const idx = lower.indexOf(q);
    if (idx === -1) return -1;
    return idx === 0 ? 2 : 1;
}

function scoreItem(item, query) {
    const labelScore = fuzzyMatch(item.label, query);
    if (labelScore > 0) return labelScore + 1;
    const kwScore = fuzzyMatch(item.keywords || '', query);
    if (kwScore > 0) return kwScore;
    const pathScore = fuzzyMatch(item.path, query);
    if (pathScore > 0) return pathScore;
    return -1;
}

const CommandPalette = ({ open, onClose }) => {
    const [query, setQuery] = useState('');
    const [dynamicItems, setDynamicItems] = useState([]);
    const navigate = useNavigate();
    const { command_palette: pluginPaletteItems } = useContributions();

    // Fetch dynamic items (services/containers + servers) when opened
    useEffect(() => {
        if (!open) return;
        setQuery('');

        let cancelled = false;
        async function fetchDynamic() {
            const items = [];
            try {
                const containers = await api.getContainers();
                if (!cancelled && Array.isArray(containers)) {
                    containers.forEach(c => {
                        items.push({
                            label: c.name || c.Names?.[0]?.replace(/^\//, ''),
                            path: `/docker`,
                            category: 'Containers',
                            keywords: `${c.Image || ''} ${c.State || ''}`,
                        });
                    });
                }
            } catch (_e) {} // eslint-disable-line no-empty
            try {
                const serverData = await api.getServers();
                const servers = serverData?.servers || serverData || [];
                if (!cancelled && Array.isArray(servers)) {
                    servers.forEach(s => {
                        items.push({
                            label: s.name || s.hostname,
                            path: `/servers/${s.id}`,
                            category: 'Servers',
                            keywords: `${s.hostname || ''} ${s.ip_address || ''}`,
                        });
                    });
                }
            } catch (_e) {} // eslint-disable-line no-empty
            if (!cancelled) setDynamicItems(items);
        }
        fetchDynamic();
        return () => { cancelled = true; };
    }, [open]);

    const allItems = useMemo(() => {
        const fromPlugins = (pluginPaletteItems || [])
            .filter((it) => it && it.label && it.path)
            .map((it) => ({
                label: it.label,
                path: it.path,
                category: it.category || 'Extensions',
                keywords: it.keywords || '',
            }));
        return [...STATIC_PAGES, ...fromPlugins, ...dynamicItems];
    }, [dynamicItems, pluginPaletteItems]);

    const results = useMemo(() => {
        if (!query.trim()) return allItems.slice(0, 20);
        return allItems
            .map(item => ({ ...item, score: scoreItem(item, query.trim()) }))
            .filter(item => item.score > 0)
            .sort((a, b) => b.score - a.score)
            .slice(0, 20);
    }, [query, allItems]);

    const handleSelect = useCallback((item) => {
        navigate(item.path);
        onClose();
    }, [navigate, onClose]);

    // Group results by category
    const grouped = useMemo(() => {
        const groups = {};
        results.forEach(item => {
            if (!groups[item.category]) groups[item.category] = [];
            groups[item.category].push(item);
        });
        return groups;
    }, [results]);

    return (
        <CommandDialog open={open} onOpenChange={(isOpen) => { if (!isOpen) onClose(); }}>
            <CommandInput
                placeholder="Search pages, services, servers..."
                value={query}
                onValueChange={setQuery}
            />
            <CommandList>
                <CommandEmpty>No results found</CommandEmpty>
                {Object.entries(grouped).map(([category, items]) => (
                    <CommandGroup key={category} heading={category}>
                        {items.map(item => (
                            <CommandItem
                                key={`${item.category}-${item.path}-${item.label}`}
                                value={`${item.label} ${item.path} ${item.keywords || ''}`}
                                onSelect={() => handleSelect(item)}
                            >
                                <span className="command-palette__item-label">{item.label}</span>
                                <span className="command-palette__item-path">{item.path}</span>
                            </CommandItem>
                        ))}
                    </CommandGroup>
                ))}
            </CommandList>
        </CommandDialog>
    );
};

export default CommandPalette;
