import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Pill } from '@/components/ds';
import { useDeployments } from '../../hooks/useDeployments';
import { getDeployStatus, formatRelativeTime, formatDuration } from '../../utils/serviceTypes';
import EmptyState from '../EmptyState';

// Deployment status → semantic tone (ds Pill kind / dot modifier)
const DEPLOY_TONE = {
    success: 'green',
    failed: 'red',
    in_progress: 'amber',
    rolled_back: 'gray',
    pending: 'cyan',
};

const EventsTab = ({ appId }) => {
    const { deployments, loading, error, reload } = useDeployments(appId);
    const [expandedId, setExpandedId] = useState(null);

    if (loading) {
        return <EmptyState loading title="Loading deployment history..." />;
    }

    if (error) {
        return (
            <div className="events-tab__empty">
                <h3>Failed to load events</h3>
                <p>{error}</p>
                <Button variant="outline" onClick={reload}>Retry</Button>
            </div>
        );
    }

    if (deployments.length === 0) {
        return (
            <div className="events-tab__empty">
                <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" style={{ opacity: 0.4 }}>
                    <circle cx="12" cy="12" r="10"/>
                    <polyline points="12 6 12 12 16 14"/>
                </svg>
                <h3>No deployments yet</h3>
                <p>Deploy your service to see events here.</p>
            </div>
        );
    }

    return (
        <div className="events-tab">
            <div className="events-tab__header">
                <h3 className="svc-eyebrow">
                    Deployment Events <span className="svc-eyebrow__count">&middot; {deployments.length}</span>
                </h3>
                <Button variant="outline" size="sm" onClick={reload}>Refresh</Button>
            </div>

            <div className="events-tab__timeline">
                {deployments.map((deploy, idx) => {
                    const statusInfo = getDeployStatus(deploy.status);
                    const tone = DEPLOY_TONE[deploy.status] || 'cyan';
                    const isExpanded = expandedId === deploy.id;
                    const isLatest = idx === 0 && deploy.status === 'success';

                    return (
                        <div
                            key={deploy.id}
                            className={`events-tab__event ${isExpanded ? 'events-tab__event--expanded' : ''}`}
                            onClick={() => setExpandedId(isExpanded ? null : deploy.id)}
                        >
                            <div className={`events-tab__event-status events-tab__event-status--${tone}`} />
                            <div className="events-tab__event-body">
                                <div className="events-tab__event-header">
                                    <div className="events-tab__event-commit">
                                        {deploy.commitSha && (
                                            <span className="events-tab__event-sha">
                                                {deploy.commitSha.substring(0, 7)}
                                            </span>
                                        )}
                                        <span className="events-tab__event-message">
                                            {deploy.commitMessage || deploy.version || `Deployment #${deployments.length - idx}`}
                                        </span>
                                    </div>
                                    <Pill kind={isLatest ? 'green' : tone}>
                                        {isLatest ? 'Live' : statusInfo.label}
                                    </Pill>
                                </div>
                                <div className="events-tab__event-meta">
                                    {deploy.duration && <span>{formatDuration(deploy.duration)}</span>}
                                    {deploy.trigger && <span>{deploy.trigger}</span>}
                                    {deploy.branch && <span>{deploy.branch}</span>}
                                    <span>{formatRelativeTime(deploy.timestamp)}</span>
                                </div>
                                {isExpanded && deploy.logs && (
                                    <div className="events-tab__event-logs">
                                        <pre>{deploy.logs}</pre>
                                    </div>
                                )}
                            </div>
                        </div>
                    );
                })}
            </div>
        </div>
    );
};

export default EventsTab;
