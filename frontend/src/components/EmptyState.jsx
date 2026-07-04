import { Inbox } from 'lucide-react';
import { Skeleton } from './Skeleton';

export default function EmptyState({
    icon: Icon = Inbox,
    title = 'No items found',
    description = '',
    action = null,
    size = 'default',
    loading = false
}) {
    if (loading) {
        // Skeleton placeholder (not a spinner): a generic panel shape that reads
        // as "content is loading" across the ~40 page/section loaders that use it.
        return (
            <div
                className={`empty-state empty-state--${size} empty-state--loading`}
                role="status"
                aria-busy="true"
                aria-label={title || 'Loading'}
            >
                <div className="skeleton-panel">
                    <div className="skeleton-panel__head">
                        <Skeleton variant="avatar" />
                        <div className="skeleton-panel__head-text">
                            <Skeleton variant="title" width="42%" />
                            <Skeleton variant="line" width="26%" />
                        </div>
                    </div>
                    <div className="skeleton-panel__cards">
                        <Skeleton variant="card" />
                        <Skeleton variant="card" />
                        <Skeleton variant="card" />
                    </div>
                    <div className="skeleton-panel__rows">
                        <Skeleton variant="line" width="100%" />
                        <Skeleton variant="line" width="92%" />
                        <Skeleton variant="line" width="76%" />
                    </div>
                </div>
            </div>
        );
    }

    return (
        <div className={`empty-state empty-state--${size}`}>
            <div className="empty-state__icon">
                <Icon size={size === 'lg' ? 64 : 48} />
            </div>
            <h3 className="empty-state__title">{title}</h3>
            {description && (
                <p className="empty-state__description">{description}</p>
            )}
            {action && (
                <div className="empty-state__action">{action}</div>
            )}
        </div>
    );
}
