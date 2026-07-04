import { Activity, ScrollText } from 'lucide-react';

// Shared sub-nav for the Observability page group — Monitoring (live metrics +
// alerts) and Events (the unified SystemEvent stream, formerly "Telemetry").
// Rendered in each page's <PageTopbar tabs={MONITOR_TABS}>. Fleet Monitor stays
// under the Servers group (it is server-scoped); GPU is its own hardware page.
// The Status Pages tab is contributed by the serverkit-status builtin extension
// (tab-group contribution, #43) and merged in by TabGroupLayout
// groupId="monitoring".
export const MONITOR_TABS = [
    { to: '/monitoring', label: 'Monitoring', end: true, icon: <Activity size={15} /> },
    { to: '/telemetry', label: 'Events', icon: <ScrollText size={15} /> },
];
