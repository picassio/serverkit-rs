
export function ProgressBar({ percent = 0, color, className = '' }) {
    const clamped = Math.max(0, Math.min(100, Number(percent) || 0));
    const fillStyle = { width: `${clamped}%`, ...(color && { background: color }) };
    return (
        <div className={`progress-bar ${className}`.trim()}>
            <div className="progress-fill" style={fillStyle} />
        </div>
    );
}

export default ProgressBar;
