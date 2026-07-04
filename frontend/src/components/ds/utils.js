// Helpers for the design-system primitives (see docs/REDESIGN_MAP.md §3).

// Deterministic gradient from a name — used by ServiceTile and elsewhere a
// colored initial avatar is needed. Same name always yields the same hue.
export function svcGrad(name = '') {
    let h = 0;
    for (let i = 0; i < name.length; i++) {
        h = (h * 31 + name.charCodeAt(i)) % 360;
    }
    return `linear-gradient(150deg, hsl(${h} 58% 60%), hsl(${(h + 24) % 360} 55% 46%))`;
}

// First letter (uppercased) for an avatar/tile label.
export function initials(name = '') {
    return (name.trim()[0] || '?').toUpperCase();
}

// Threshold color for a percentage gauge (red > 75, amber > 50, else accent).
export function gaugeColor(pct) {
    if (pct > 75) return 'var(--red)';
    if (pct > 50) return 'var(--amber)';
    return 'var(--accent-bright)';
}
