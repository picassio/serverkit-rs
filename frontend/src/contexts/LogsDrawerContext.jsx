import { createContext, useContext, useState, useCallback } from 'react';

const LogsDrawerContext = createContext(null);

export function LogsDrawerProvider({ children }) {
    const [drawerState, setDrawerState] = useState('closed'); // 'closed' | 'collapsed' | 'expanded'
    const [service, setService] = useState(null); // { name, containerId, logPath, appType }

    const openDrawer = useCallback((serviceInfo) => {
        setService(serviceInfo);
        setDrawerState('expanded');
    }, []);

    const closeDrawer = useCallback(() => {
        setDrawerState('closed');
        setService(null);
    }, []);

    const toggleDrawer = useCallback(() => {
        setDrawerState(prev => {
            if (prev === 'closed') return 'expanded';
            if (prev === 'expanded') return 'collapsed';
            return 'expanded';
        });
    }, []);

    const collapseDrawer = useCallback(() => {
        setDrawerState('collapsed');
    }, []);

    const expandDrawer = useCallback(() => {
        setDrawerState('expanded');
    }, []);

    return (
        <LogsDrawerContext.Provider value={{ drawerState, service, openDrawer, closeDrawer, toggleDrawer, collapseDrawer, expandDrawer }}>
            {children}
        </LogsDrawerContext.Provider>
    );
}

export function useLogsDrawer() {
    const ctx = useContext(LogsDrawerContext);
    if (!ctx) throw new Error('useLogsDrawer must be used within LogsDrawerProvider');
    return ctx;
}
