// Canonical relative-time helpers for the whole frontend.
//
// Two output styles existed across the codebase; keep both, but in one place:
//   - timeAgo(iso)          compact: "just now", "4m", "3h", "2d", else a date
//   - formatRelativeTime    verbose: "just now", "4m ago", "3h ago", "2d ago"
//
// Replaces the scattered local copies in Dashboard.jsx, serviceTypes.js,
// logHelpers.js, and the wordpress/* components.

// Compact relative time, e.g. "just now", "4m", "3h", "2d", else a date.
export function timeAgo(iso) {
    if (!iso) return '';
    const then = new Date(iso).getTime();
    if (Number.isNaN(then)) return '';
    const seconds = Math.floor((Date.now() - then) / 1000);
    if (seconds < 45) return 'just now';
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h`;
    const days = Math.floor(hours / 24);
    if (days < 7) return `${days}d`;
    return new Date(then).toLocaleDateString();
}

// Verbose relative time, e.g. "just now", "4m ago", "3h ago", "2d ago",
// else a localized date once past ~30 days.
export function formatRelativeTime(iso) {
    if (!iso) return '';
    const date = new Date(iso);
    const then = date.getTime();
    if (Number.isNaN(then)) return '';
    const diffSec = Math.floor((Date.now() - then) / 1000);
    const diffMin = Math.floor(diffSec / 60);
    const diffHour = Math.floor(diffMin / 60);
    const diffDay = Math.floor(diffHour / 24);

    if (diffSec < 60) return 'just now';
    if (diffMin < 60) return `${diffMin}m ago`;
    if (diffHour < 24) return `${diffHour}h ago`;
    if (diffDay < 30) return `${diffDay}d ago`;
    return date.toLocaleDateString();
}

// Humanize a duration in seconds, e.g. "45s", "3m 20s".
export function formatDuration(seconds) {
    if (!seconds || seconds < 0) return '-';
    if (seconds < 60) return `${Math.round(seconds)}s`;
    const min = Math.floor(seconds / 60);
    const sec = Math.round(seconds % 60);
    return `${min}m ${sec}s`;
}

export default timeAgo;
