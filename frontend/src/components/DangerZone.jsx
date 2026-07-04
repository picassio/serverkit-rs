
export function DangerZone({ title, description, action, children, className = '' }) {
    if (children) {
        return <div className={`danger-zone ${className}`.trim()}>{children}</div>;
    }
    return (
        <div className={`danger-zone ${className}`.trim()}>
            <div>
                {title && <h4>{title}</h4>}
                {description && <p>{description}</p>}
            </div>
            {action}
        </div>
    );
}

export default DangerZone;
