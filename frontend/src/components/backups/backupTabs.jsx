import { Archive, Clock, Cloud, Settings } from 'lucide-react';

// Shared sub-nav for the Backups page group (Backups / Schedules / Storage /
// Settings). Rendered in the shared PageTopbar tabs so the sections live in the
// page top bar — matching the Domains/Services top-bar layout (see
// docs/REDESIGN_MAP.md §6 decision 3). All sections render from the single
// Backups page, keyed off the /backups/:tab route.
export const BACKUP_TABS = [
    { to: '/backups', label: 'Backups', end: true, icon: <Archive size={15} /> },
    { to: '/backups/schedules', label: 'Schedules', icon: <Clock size={15} /> },
    { to: '/backups/storage', label: 'Storage', icon: <Cloud size={15} /> },
    { to: '/backups/settings', label: 'Settings', icon: <Settings size={15} /> },
];
