import { useEffect, useState } from 'react';
import { Building2 } from 'lucide-react';
import { api } from '../services/api';
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from './ui/select';

const ACTIVE_KEY = 'active_workspace_id';
const ACTIVE_WS_KEY = 'active_workspace';  // full workspace object for nav/brand settings
const ACCENT_KEY = 'workspace_accent';  // ThemeContext applies this with precedence over the user accent

// Mirror a workspace's brand color into localStorage so ThemeContext can apply it
// (with precedence over the user's accent), or clear it when there's no color.
function syncAccent(ws) {
    if (ws && ws.primary_color) {
        localStorage.setItem(ACCENT_KEY, ws.primary_color);
    } else {
        localStorage.removeItem(ACCENT_KEY);
    }
}

function syncWorkspace(ws) {
    if (ws) {
        localStorage.setItem(ACTIVE_WS_KEY, JSON.stringify(ws));
    } else {
        localStorage.removeItem(ACTIVE_WS_KEY);
    }
    syncAccent(ws);
}

// Active-workspace selector (#33). Self-contained: it reads/writes the active
// workspace in localStorage (which services/api/client.js sends ambiently as the
// X-Workspace-Id header) and reloads so every page re-fetches its lists under the
// new scope. Always rendered when at least one workspace exists so the scoping
// concept stays visible even on single-workspace installs.
const WorkspaceSwitcher = () => {
    const [workspaces, setWorkspaces] = useState([]);
    const [active, setActive] = useState(() => localStorage.getItem(ACTIVE_KEY) || 'all');

    useEffect(() => {
        let alive = true;
        api.getWorkspaces()
            .then((res) => {
                if (!alive) return;
                const list = res?.workspaces || [];
                setWorkspaces(list);
                const stored = localStorage.getItem(ACTIVE_KEY);
                if (stored && stored !== 'all') {
                    const ws = list.find((w) => String(w.id) === stored);
                    if (!ws) {
                        // Stale selection (workspace deleted / access lost): clear it so
                        // a dead X-Workspace-Id header / brand color isn't applied.
                        localStorage.removeItem(ACTIVE_KEY);
                        localStorage.removeItem(ACTIVE_WS_KEY);
                        localStorage.removeItem(ACCENT_KEY);
                        setActive('all');
                    } else {
                        // Keep workspace settings (brand color, nav permissions) fresh.
                        syncWorkspace(ws);
                    }
                }
            })
            .catch(() => { /* best-effort; the selector just won't render */ });
        return () => { alive = false; };
    }, []);

    if (workspaces.length === 0) return null;

    const handleChange = (value) => {
        if (value === 'all') {
            localStorage.removeItem(ACTIVE_KEY);
            localStorage.removeItem(ACTIVE_WS_KEY);
            localStorage.removeItem(ACCENT_KEY);
        } else {
            const ws = workspaces.find((w) => String(w.id) === value);
            localStorage.setItem(ACTIVE_KEY, value);
            syncWorkspace(ws);
        }
        // Reload so every page re-fetches its lists (and re-applies the brand color)
        // under the new workspace scope.
        window.location.reload();
    };

    return (
        <Select value={active} onValueChange={handleChange}>
            <SelectTrigger className="workspace-switcher__trigger" aria-label="Active workspace">
                <Building2 size={14} className="workspace-switcher__icon" aria-hidden="true" />
                <SelectValue placeholder="All workspaces" />
            </SelectTrigger>
            <SelectContent>
                <SelectItem value="all">All workspaces</SelectItem>
                {workspaces.map((w) => (
                    <SelectItem key={w.id} value={String(w.id)}>{w.name}</SelectItem>
                ))}
            </SelectContent>
        </Select>
    );
};

export default WorkspaceSwitcher;
