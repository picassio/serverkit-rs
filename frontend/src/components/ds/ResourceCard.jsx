import { ChevronRight } from 'lucide-react';
import { cn } from '@/lib/utils';

/**
 * Standard resource list item: icon, title/subtitle, status, metadata chips,
 * and optional actions. Used for server rows, app rows, domain rows, container
 * rows, and any other "thing with a name and state" list.
 *
 *   <ResourceCard
 *     icon={<Server size={18} />}
 *     title="web-01"
 *     subtitle="192.168.1.10"
 *     status={<Pill kind="green">online</Pill>}
 *     meta={[{ label: 'OS', value: 'Ubuntu 22.04' }, { label: 'Agent', value: '1.2.3' }]}
 *     actions={<Button size="sm">Manage</Button>}
 *     onClick={() => navigate('/servers/1')}
 *   />
 */
export function ResourceCard({
    icon,
    title,
    subtitle,
    status,
    meta = [],
    actions,
    onClick,
    className,
    selected = false,
    selection,
}) {
    return (
        <div
            className={cn(
                'sk-resource-card',
                onClick && 'is-clickable',
                selected && 'is-selected',
                className
            )}
            onClick={onClick}
            role={onClick ? 'button' : undefined}
            tabIndex={onClick ? 0 : undefined}
        >
            {selection && (
                <div className="sk-resource-card__select" onClick={(e) => e.stopPropagation()}>
                    {selection}
                </div>
            )}

            {icon && <div className="sk-resource-card__icon">{icon}</div>}

            <div className="sk-resource-card__identity">
                <div className="sk-resource-card__title-row">
                    <span className="sk-resource-card__title">{title}</span>
                    {subtitle && <span className="sk-resource-card__subtitle">{subtitle}</span>}
                </div>
                {meta.length > 0 && (
                    <div className="sk-resource-card__meta">
                        {meta.map((item, i) => (
                            <span key={item.label || i} className="sk-resource-card__meta-item">
                                <span className="sk-resource-card__meta-label">{item.label}</span>
                                <span className="sk-resource-card__meta-value">{item.value}</span>
                            </span>
                        ))}
                    </div>
                )}
            </div>

            <div className="sk-resource-card__status">{status}</div>

            {actions && <div className="sk-resource-card__actions">{actions}</div>}

            {onClick && <ChevronRight size={16} className="sk-resource-card__chevron" />}
        </div>
    );
}

/**
 * Wrapper that lays out ResourceCards in a consistent vertical stack.
 */
export function ResourceList({ children, className }) {
    return <div className={cn('sk-resource-list', className)}>{children}</div>;
}

export default ResourceCard;
