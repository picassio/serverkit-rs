import { createContext, useContext, useState, useEffect, useCallback } from 'react';

const LayoutContext = createContext(null);

// Shell geometry options, kept in sync with the switcher in Sidebar and the
// [data-layout="..."] rules in styles/layout/_sidebar.scss. 'topbar' is reserved
// for a later phase; any unknown or stale stored value falls back to 'sidebar'.
const LAYOUTS = ['sidebar', 'rail', 'topbar'];
const DEFAULT_LAYOUT = 'sidebar';

const normalize = (value) => (LAYOUTS.includes(value) ? value : DEFAULT_LAYOUT);

export function LayoutProvider({ children }) {
    const [layout, setLayoutState] = useState(() => normalize(localStorage.getItem('layout')));

    const setLayout = useCallback((next) => {
        const value = normalize(next);
        setLayoutState(value);
        localStorage.setItem('layout', value);
        document.documentElement.setAttribute('data-layout', value);
    }, []);

    // Mirror the persisted choice onto the document (matches ThemeContext).
    useEffect(() => {
        document.documentElement.setAttribute('data-layout', layout);
    }, [layout]);

    return (
        <LayoutContext.Provider value={{ layout, setLayout }}>
            {children}
        </LayoutContext.Provider>
    );
}

export function useLayout() {
    const context = useContext(LayoutContext);
    if (!context) {
        throw new Error('useLayout must be used within a LayoutProvider');
    }
    return context;
}

export default LayoutContext;
