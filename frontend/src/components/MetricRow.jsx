
export function MetricRow({ children, className = '' }) {
    return <div className={`metric-row ${className}`.trim()}>{children}</div>;
}

export function MetricItem({ label, value, children, className = '' }) {
    return (
        <div className={`metric-item ${className}`.trim()}>
            <span className="text-tertiary text-sm">{label}</span>
            {children ?? <span className="metric-value">{value}</span>}
        </div>
    );
}

export default MetricRow;
