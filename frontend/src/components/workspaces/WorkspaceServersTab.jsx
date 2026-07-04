import { useNavigate } from 'react-router-dom';
import { Server, Plus } from 'lucide-react';
import { ServiceTile, Pill } from '@/components/ds';
import { Button } from '@/components/ui/button';
import EmptyState from '../EmptyState';

const SERVER_PILL = { online: 'green', pending: 'amber', offline: 'red' };

const WorkspaceServersTab = ({ wsId, srvIn, srvOut, onMoveServer }) => {
    const navigate = useNavigate();

    return (
        <>
            {srvIn.length === 0 ? (
                <EmptyState icon={Server} title="No servers in this workspace yet" description="Move one in below." />
            ) : (
                <div className="ws-detail__tablecard">
                    <table className="sk-dtable">
                        <thead><tr><th>Server</th><th>Status</th><th style={{ width: 160 }} /></tr></thead>
                        <tbody>
                            {srvIn.map(s => (
                                <tr key={s.id} className="is-clickable" onClick={() => navigate(`/servers/${s.id}`)}>
                                    <td>
                                        <div className="sk-cell-name">
                                            <ServiceTile name={s.name} size={30} />
                                            <div>
                                                <div>{s.name}</div>
                                                <div className="sk-cell-sub">{s.ip_address || s.hostname || ''}</div>
                                            </div>
                                        </div>
                                    </td>
                                    <td><Pill kind={SERVER_PILL[s.status] || 'gray'}>{s.status || 'unknown'}</Pill></td>
                                    <td onClick={e => e.stopPropagation()}>
                                        <div className="ws-detail__rowactions">
                                            <Button size="sm" variant="destructive" onClick={() => onMoveServer(s.id, null)}>Remove</Button>
                                        </div>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            )}
            {srvOut.length > 0 && (
                <>
                    <div className="ws-pick-label">Move a server into this workspace</div>
                    <div className="ws-pick">
                        {srvOut.map(s => (
                            <div key={s.id} className="ws-pick__item" onClick={() => onMoveServer(s.id, wsId)}>
                                <ServiceTile name={s.name} size={28} className="ws-pick__tile" />
                                <span className="ws-pick__name">{s.name}</span>
                                {s.ip_address && <span className="sk-tag">{s.ip_address}</span>}
                                <Plus size={16} className="ws-pick__plus" />
                            </div>
                        ))}
                    </div>
                </>
            )}
        </>
    );
};

export default WorkspaceServersTab;
