import { useParams, useNavigate } from 'react-router-dom';

/**
 * Syncs the active tab with a URL parameter.
 * @param {string} basePath - The base route path (e.g., '/security', '/apps/5')
 * @param {string[]} validTabs - List of valid tab slugs
 * @param {string} defaultTab - Fallback tab when URL param is missing/invalid
 * @returns {[string, (tab: string) => void]} - [activeTab, setActiveTab]
 */
export default function useTabParam(basePath, validTabs, defaultTab = validTabs[0]) {
    const { tab } = useParams();
    const navigate = useNavigate();

    const activeTab = validTabs.includes(tab) ? tab : defaultTab;
    const setActiveTab = (t) => navigate(`${basePath}/${t}`, { replace: true });

    return [activeTab, setActiveTab];
}
