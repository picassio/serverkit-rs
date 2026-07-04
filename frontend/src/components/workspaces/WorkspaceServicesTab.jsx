import { useNavigate } from 'react-router-dom';
import { Box, Plus } from 'lucide-react';
import { ServiceTile, Pill } from '@/components/ds';
import { Button } from '@/components/ui/button';
import EmptyState from '../EmptyState';

const APP_PILL = { running: 'green', stopped: 'gray', failed: 'red' };

const WorkspaceServicesTab = ({ wsId, services, appsOut, onMoveApp, onShare }) => {
    const navigate = useNavigate();

    return (
        <>
            {services.length === 0 ? (
                <EmptyState icon={Box} title="No services in this workspace yet" description="Move one in below." />
            ) : (
                <div className="ws-detail__tablecard">
                    <table className="sk-dtable">
                        <thead><tr><th>Service</th><th>Status</th><th style={{ width: 220 }} /></tr></thead>
                        <tbody>
                            {services.map(a => (
                                <tr key={a.id} className="is-clickable" onClick={() => navigate(`/services/${a.id}`)}>
                                    <td>
                                        <div className="sk-cell-name">
                                            <ServiceTile name={a.name} size={30} />
                                            <div>
                                                <div>{a.name}</div>
                                                <div className="sk-cell-sub">{a.app_type}{a.domain ? ` · ${a.domain}` : ''}</div>
                                            </div>
                                        </div>
                                    </td>
                                    <td><Pill kind={APP_PILL[a.status] || 'amber'}>{a.status || 'unknown'}</Pill></td>
                                    <td onClick={e => e.stopPropagation()}>
                                        <div className="ws-detail__rowactions">
                                            <Button size="sm" variant="outline" onClick={() => onShare(a)}>Share</Button>
                                            <Button size="sm" variant="destructive" onClick={() => onMoveApp(a.id, null)}>Remove</Button>
                                        </div>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            )}
            {appsOut.length > 0 && (
                <>
                    <div className="ws-pick-label">Move an application into this workspace</div>
                    <div className="ws-pick">
                        {appsOut.map(a => (
                            <div key={a.id} className="ws-pick__item" onClick={() => onMoveApp(a.id, wsId)}>
                                <ServiceTile name={a.name} size={28} className="ws-pick__tile" />
                                <span className="ws-pick__name">{a.name}</span>
                                <span className="sk-tag">{a.app_type}</span>
                                <Plus size={16} className="ws-pick__plus" />
                            </div>
                        ))}
                    </div>
                </>
            )}
        </>
    );
};

export default WorkspaceServicesTab;
