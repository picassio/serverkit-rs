import { cn } from '@/lib/utils';

// Tiny inline trend line. `data` is an array of numbers.
export function Sparkline({ data = [], color = 'var(--accent-bright)', width = 120, height = 34, className }) {
    if (!data.length) return null;
    const max = Math.max(...data);
    const min = Math.min(...data);
    const span = max - min || 1;
    const pts = data
        .map((v, i) => `${(i / (data.length - 1 || 1)) * width},${height - ((v - min) / span) * (height - 4) - 2}`)
        .join(' ');
    return (
        <svg className={cn('sk-spark', className)} width={width} height={height} aria-hidden="true">
            <polyline
                points={pts}
                fill="none"
                stroke={color}
                strokeWidth="1.6"
                strokeLinejoin="round"
                strokeLinecap="round"
            />
        </svg>
    );
}

export default Sparkline;
