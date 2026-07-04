
const ContributionGraph = ({ data, title, username }) => {
    if (!data || data.length === 0) return null;

    const maxCount = Math.max(...data.map(d => d.count), 1);
    
    // Group by week for display
    const weeks = [];
    let currentWeek = [];
    
    data.forEach((day, index) => {
        currentWeek.push(day);
        if (currentWeek.length === 7 || index === data.length - 1) {
            weeks.push(currentWeek);
            currentWeek = [];
        }
    });

    const getLevel = (count) => {
        if (count === 0) return 0;
        const percentage = (count / maxCount) * 100;
        if (percentage <= 25) return 1;
        if (percentage <= 50) return 2;
        if (percentage <= 75) return 3;
        return 4;
    };

    const formatDate = (dateStr) => {
        const date = new Date(dateStr);
        return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
    };

    return (
        <div className="contribution-graph-container">
            <div className="graph-header">
                <h4>{title} {username && <span className="username">@{username}</span>}</h4>
                <div className="graph-legend">
                    <span>Less</span>
                    <div className="legend-cells">
                        <div className="cell level-0" />
                        <div className="cell level-1" />
                        <div className="cell level-2" />
                        <div className="cell level-3" />
                        <div className="cell level-4" />
                    </div>
                    <span>More</span>
                </div>
            </div>
            <div className="graph-grid-wrapper">
                <div className="graph-grid">
                    {weeks.map((week, weekIndex) => (
                        <div key={weekIndex} className="graph-week">
                            {week.map((day, dayIndex) => (
                                <div
                                    key={dayIndex}
                                    className={`graph-cell level-${getLevel(day.count)}`}
                                    title={`${day.count} actions on ${formatDate(day.date)}`}
                                />
                            ))}
                        </div>
                    ))}
                </div>
            </div>
        </div>
    );
};

export default ContributionGraph;
