import { useState, useEffect } from 'react';

// Subscribe to a CSS media query and re-render when it changes.
// Returns the current match as a boolean. SSR-safe (defaults to false
// when matchMedia is unavailable).
export default function useMediaQuery(query) {
    const getMatch = () => (
        typeof window !== 'undefined' && typeof window.matchMedia === 'function'
            ? window.matchMedia(query).matches
            : false
    );

    const [matches, setMatches] = useState(getMatch);

    useEffect(() => {
        if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
            return undefined;
        }
        const mql = window.matchMedia(query);
        const onChange = (e) => setMatches(e.matches);

        // Sync immediately in case the query changed between render and effect.
        setMatches(mql.matches);
        mql.addEventListener('change', onChange);
        return () => mql.removeEventListener('change', onChange);
    }, [query]);

    return matches;
}
