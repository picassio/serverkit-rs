// statusInfo.dotClass → ds Pill kind
export const STATUS_PILL = {
    live: 'green',
    stopped: 'gray',
    deploying: 'amber',
    building: 'amber',
    failed: 'red',
};

// Ingress plane → label + Pill kind. Host Nginx is the neutral default;
// a managed proxy stack reads as the accent (cyan) choice. NULL/undefined
// reads as host Nginx, matching the backend.
export const INGRESS_META = {
    proxy_stack: { label: 'Proxy stack', kind: 'cyan' },
    nginx: { label: 'Nginx', kind: 'gray' },
};
