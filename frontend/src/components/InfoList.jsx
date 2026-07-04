
export function InfoList({ children, className = '', style }) {
    return <div className={`info-list ${className}`.trim()} style={style}>{children}</div>;
}

export function InfoItem({ label, value, mono = false, children }) {
    const valueClass = ['info-value', mono && 'mono'].filter(Boolean).join(' ');
    return (
        <div className="info-item">
            <span className="info-label">{label}</span>
            {children ?? <span className={valueClass}>{value}</span>}
        </div>
    );
}

export default InfoList;
