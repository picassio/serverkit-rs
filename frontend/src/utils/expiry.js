// Human-friendly expiry formatting for registration / certificate dates. Returns
// null for a missing or unparseable date, otherwise both an exact date and a
// relative phrase plus an urgency tone, so callers can show e.g. "439 days left"
// with the real date ("Mar 4, 2027") on a second line or in a tooltip.
export function formatExpiry(iso) {
    if (!iso) return null;
    const date = new Date(iso);
    if (Number.isNaN(date.getTime())) return null;

    const days = Math.round((date.getTime() - Date.now()) / 86400000);
    const absolute = date.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });

    let relative;
    if (days < 0) relative = `Expired ${Math.abs(days)}d ago`;
    else if (days === 0) relative = 'Expires today';
    else if (days < 60) relative = `${days} days left`;
    else if (days < 365) relative = `${Math.round(days / 30)} months left`;
    else {
        const years = Math.floor(days / 365);
        const months = Math.round((days % 365) / 30);
        relative = months ? `${years}y ${months}mo left` : `${years} year${years > 1 ? 's' : ''} left`;
    }

    const tone = days < 0 ? 'red' : days <= 30 ? 'amber' : 'green';
    return { days, absolute, relative, tone };
}

export default formatExpiry;
