import { cn } from '@/lib/utils';

// Circular score ring (security posture / sender reputation). Renders a
// background track + a colored arc proportional to value/max.
export function ScoreGauge({ value = 0, max = 100, size = 120, stroke = 10, color = 'var(--green)', label, className }) {
    const pct = Math.max(0, Math.min(1, value / (max || 1)));
    const r = (size - stroke) / 2;
    const circ = 2 * Math.PI * r;
    return (
        <div className={cn('sk-score', className)} style={{ width: size, height: size }}>
            <svg className="sk-score__svg" width={size} height={size}>
                <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="var(--surface-3)" strokeWidth={stroke} />
                <circle
                    cx={size / 2}
                    cy={size / 2}
                    r={r}
                    fill="none"
                    stroke={color}
                    strokeWidth={stroke}
                    strokeDasharray={circ}
                    strokeDashoffset={circ * (1 - pct)}
                    strokeLinecap="round"
                />
            </svg>
            <div className="sk-score__center">
                <div className="sk-score__num" style={{ fontSize: size * 0.26 }}>{value}</div>
                {label && <div className="sk-score__label">{label}</div>}
            </div>
        </div>
    );
}

export default ScoreGauge;
