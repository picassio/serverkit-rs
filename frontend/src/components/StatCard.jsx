
// ─────────────────────────────────────────────────────────────────────────────
// Stat strip — a compact, instrument-panel readout bar.
//
// Replaces the old hero-metric stat cards (big number + icon chip) with a single
// dense bordered strip of segments. Use it for the page-level summary row that
// sits above a page's tabs/content.
//
//   <StatStrip>
//     <Stat label="Total Backups" value={12} />
//     <Stat label="ClamAV" value="Active" state="success" />
//   </StatStrip>
//
// `state` (success | warning | danger | info | neutral) tints the value and adds
// a status dot. Status is never carried by color alone — the value text always
// states it too, the dot is only reinforcement.
// ─────────────────────────────────────────────────────────────────────────────

const STATEFUL = new Set(['success', 'warning', 'danger', 'info']);

export function StatStrip({ children, className = '', ariaLabel }) {
    return (
        <div
            className={`stat-strip ${className}`.trim()}
            role="group"
            aria-label={ariaLabel}
        >
            {children}
        </div>
    );
}

export function Stat({
    label,
    value,
    suffix,
    detail,
    state,
    valueClassName = '',
    onClick,
    active = false,
    children,
}) {
    const hasDot = STATEFUL.has(state);
    const itemClass = [
        'stat-strip__item',
        state && `is-${state}`,
        onClick && 'stat-strip__item--clickable',
        active && 'is-active',
    ].filter(Boolean).join(' ');

    const Tag = onClick ? 'button' : 'div';

    return (
        <Tag
            className={itemClass}
            {...(onClick && { type: 'button', onClick, 'aria-pressed': active })}
        >
            <span className="stat-strip__label">{label}</span>
            <span className={`stat-strip__value ${valueClassName}`.trim()}>
                {hasDot && <span className="stat-strip__dot" aria-hidden="true" />}
                {children ?? value}
                {suffix && <span className="stat-strip__suffix">{suffix}</span>}
            </span>
            {detail && <span className="stat-strip__detail">{detail}</span>}
        </Tag>
    );
}

// ── Back-compat aliases ──────────────────────────────────────────────────────
// Existing call sites import { StatCard, StatsGrid }. They now render the strip;
// the old icon/iconVariant props are intentionally ignored (the strip drops the
// decorative icon chips). No call-site changes required for these.

export function StatsGrid({ children, className = '', ...rest }) {
    return <StatStrip className={className} {...rest}>{children}</StatStrip>;
}

export function StatCard({ icon: _icon, iconVariant: _iconVariant, iconNode: _iconNode, ...rest }) {
    return <Stat {...rest} />;
}

export default StatCard;
