import { Box, Plus, LayoutTemplate, Activity } from 'lucide-react';

// Shared sub-nav for the Services page group (Services / New Service /
// Templates / Deploy Activity). Rendered in each page's
// <PageTopbar tabs={SERVICE_TABS}> — the demo's top-bar layout replaces the old
// sidebar sub-menu (see docs/REDESIGN_MAP.md §6 decision 3).
export const SERVICE_TABS = [
    { to: '/services', label: 'Services', end: true, icon: <Box size={15} /> },
    { to: '/services/new', label: 'New Service', icon: <Plus size={15} /> },
    { to: '/templates', label: 'Templates', icon: <LayoutTemplate size={15} /> },
    { to: '/deployments', label: 'Deploy Activity', icon: <Activity size={15} /> },
];
