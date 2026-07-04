import { cn } from '@/lib/utils';

// Status pill: a small dot + label. Replaces ad-hoc per-page status badges.
// kind: 'green' | 'amber' | 'red' | 'cyan' | 'violet' | 'gray'
export function Pill({ kind = 'gray', dot = true, className, children, ...props }) {
    return (
        <span className={cn('sk-pill', `sk-pill--${kind}`, className)} {...props}>
            {dot && <span className="sk-pill__dot" />}
            {children}
        </span>
    );
}

export default Pill;
