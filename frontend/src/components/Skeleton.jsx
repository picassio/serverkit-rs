/**
 * Lightweight shimmer placeholder. Compose these to mirror real content while it
 * loads instead of showing a spinner.
 *
 *   <Skeleton variant="title" width="40%" />
 *   <Skeleton variant="line" />
 *   <Skeleton variant="block" height={120} />
 *
 * Variants: line | title | avatar | card | block | circle.
 */
export function Skeleton({
    variant = 'line',
    width,
    height,
    radius,
    className = '',
    style = {},
}) {
    const css = { ...style };
    const toCss = (v) => (typeof v === 'number' ? `${v}px` : v);
    if (width != null) css.width = toCss(width);
    if (height != null) css.height = toCss(height);
    if (radius != null) css.borderRadius = toCss(radius);

    return (
        <span
            className={`skeleton skeleton--${variant} ${className}`.trim()}
            style={css}
            aria-hidden="true"
        />
    );
}

export default Skeleton;
