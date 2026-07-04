import { useState, useCallback } from 'react';

const STORAGE_KEY = 'dashboard_layout';

const DEFAULT_WIDGETS = [
    { id: 'cpu', label: 'CPU', visible: true },
    { id: 'ram', label: 'RAM', visible: true },
    { id: 'network', label: 'Network', visible: true },
    { id: 'disk', label: 'Disk', visible: true },
    { id: 'chart', label: 'Metrics Chart', visible: true },
    { id: 'specs', label: 'Quick Actions & Specs', visible: true },
    { id: 'processes', label: 'Processes / Containers', visible: true },
];

function loadWidgets() {
    try {
        const stored = JSON.parse(localStorage.getItem(STORAGE_KEY));
        if (!Array.isArray(stored)) return DEFAULT_WIDGETS.map(w => ({ ...w }));

        // Merge with defaults to handle new widgets added in future versions
        const storedMap = new Map(stored.map(w => [w.id, w]));
        const merged = [];

        // Keep stored order for known widgets
        for (const sw of stored) {
            const def = DEFAULT_WIDGETS.find(d => d.id === sw.id);
            if (def) {
                merged.push({ ...def, visible: sw.visible });
            }
        }

        // Append any new defaults not in stored
        for (const dw of DEFAULT_WIDGETS) {
            if (!storedMap.has(dw.id)) {
                merged.push({ ...dw });
            }
        }

        return merged;
    } catch {
        return DEFAULT_WIDGETS.map(w => ({ ...w }));
    }
}

function saveWidgets(widgets) {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(widgets.map(({ id, visible }) => ({ id, visible }))));
}

export default function useDashboardLayout() {
    const [widgets, setWidgets] = useState(loadWidgets);

    const toggleWidget = useCallback((id) => {
        setWidgets(prev => {
            const next = prev.map(w => w.id === id ? { ...w, visible: !w.visible } : w);
            saveWidgets(next);
            return next;
        });
    }, []);

    const moveWidget = useCallback((id, direction) => {
        setWidgets(prev => {
            const idx = prev.findIndex(w => w.id === id);
            if (idx < 0) return prev;
            const swapIdx = direction === 'up' ? idx - 1 : idx + 1;
            if (swapIdx < 0 || swapIdx >= prev.length) return prev;
            const next = [...prev];
            [next[idx], next[swapIdx]] = [next[swapIdx], next[idx]];
            saveWidgets(next);
            return next;
        });
    }, []);

    const resetLayout = useCallback(() => {
        const fresh = DEFAULT_WIDGETS.map(w => ({ ...w }));
        saveWidgets(fresh);
        setWidgets(fresh);
    }, []);

    const isVisible = useCallback((id) => {
        const w = widgets.find(w => w.id === id);
        return w ? w.visible : true;
    }, [widgets]);

    return { widgets, toggleWidget, moveWidget, resetLayout, isVisible };
}
