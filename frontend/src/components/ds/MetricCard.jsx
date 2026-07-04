import { cn } from '@/lib/utils';

// KPI / stat tile: icon chip + big value (+ optional unit) + label, with an
// optional trend delta in the top-right.
// tone: 'accent' | 'green' | 'cyan' | 'amber' | 'red' | 'violet'
// trendDir: 'up' | 'down' | 'flat'
export function MetricCard({
    icon,
    tone = 'accent',
    value,
    unit,
    label,
    trend,
    trendDir = 'flat',
    className,
    children,
    ...props
}) {
    return (
        <div className={cn('sk-kpi', className)} {...props}>
            <div className="sk-kpi__top">
                {icon && <span className={cn('sk-kpi__icon', `sk-kpi__icon--${tone}`)}>{icon}</span>}
                {trend != null && (
                    <span className={cn('sk-kpi__trend', `sk-kpi__trend--${trendDir}`)}>{trend}</span>
                )}
            </div>
            <div className="sk-kpi__val">
                {value}
                {unit && <small> {unit}</small>}
            </div>
            {label && <div className="sk-kpi__label">{label}</div>}
            {children}
        </div>
    );
}

export default MetricCard;
