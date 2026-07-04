import { useState, useEffect, useCallback } from 'react';
import api from '../services/api';

// Shared, app-wide state for the optional feature modules (Email + WordPress).
// The module list is fetched once and cached at module scope so the sidebar,
// route guards, and the Settings "Modules" card all read the same source and
// stay in sync. Toggling a module (Settings) calls refresh(), which re-fetches
// and notifies every mounted consumer so the sidebar updates immediately.

let cache = null;        // last-fetched array of module objects
let inflight = null;     // de-dupes concurrent initial fetches
const listeners = new Set();

function notify() {
    for (const listener of listeners) listener(cache);
}

async function fetchModules(force = false) {
    if (cache && !force) return cache;
    if (inflight && !force) return inflight;
    inflight = api.getModules()
        .then((data) => {
            cache = data?.modules || [];
            notify();
            return cache;
        })
        .catch(() => {
            // Keep the last-known state (or empty) on a transient error.
            cache = cache || [];
            return cache;
        })
        .finally(() => { inflight = null; });
    return inflight;
}

export function useModules() {
    const [modules, setModules] = useState(cache);

    useEffect(() => {
        const listener = (m) => setModules(m);
        listeners.add(listener);
        fetchModules();
        return () => { listeners.delete(listener); };
    }, []);

    // Re-fetch and broadcast to all consumers (call after a toggle).
    const refresh = useCallback(() => fetchModules(true), []);

    // Whether a named module is enabled. Unknown / still-loading state is
    // treated as enabled so nothing flickers or hides before state loads.
    const isEnabled = useCallback((name) => {
        if (!modules) return true;
        const mod = modules.find((m) => m.name === name);
        return mod ? !!mod.enabled : true;
    }, [modules]);

    return { modules, isEnabled, refresh };
}

export default useModules;
