import { cn } from '@/lib/utils';

// Activity feed list. Compose with <FeedItem>.
export function Feed({ className, children }) {
    return <div className={cn('sk-feed', className)}>{children}</div>;
}

// A single feed row: a tinted icon chip, body content, and an optional time.
// `tone` is a semantic token name (green/cyan/amber/red/violet/accent).
export function FeedItem({ icon, tone, time, className, children }) {
    return (
        <div className={cn('sk-feed__item', className)}>
            <span className="sk-feed__dot" style={tone ? { color: `var(--${tone})` } : undefined}>
                {icon}
            </span>
            <div className="sk-feed__body">
                <div className="sk-feed__txt">{children}</div>
                {time && <div className="sk-feed__time">{time}</div>}
            </div>
        </div>
    );
}

export default Feed;
