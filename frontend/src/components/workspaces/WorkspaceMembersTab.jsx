import { Users, Plus } from 'lucide-react';
import { ServiceTile, Pill } from '@/components/ds';
import { Button } from '@/components/ui/button';

const WorkspaceMembersTab = ({ wsId, members, allUsers, onAddMember, onRemoveMember }) => (
    <>
        <div className="ws-detail__tablecard">
            <table className="sk-dtable">
                <thead><tr><th>Member</th><th>Role</th><th style={{ width: 120 }} /></tr></thead>
                <tbody>
                    {members.map(m => (
                        <tr key={m.id}>
                            <td>
                                <div className="sk-cell-name">
                                    <ServiceTile name={m.username || m.email || '?'} size={30} className="ws-row__av" />
                                    <div>
                                        <div>{m.username || m.email}</div>
                                        {m.username && m.email && <div className="sk-cell-sub">{m.email}</div>}
                                    </div>
                                </div>
                            </td>
                            <td>
                                {m.role === 'owner'
                                    ? <Pill kind="green">{m.role}</Pill>
                                    : <span className="sk-tag">{m.role}</span>}
                            </td>
                            <td>
                                {m.role !== 'owner' && (
                                    <Button size="sm" variant="destructive" onClick={() => onRemoveMember(m.id)}>Remove</Button>
                                )}
                            </td>
                        </tr>
                    ))}
                </tbody>
            </table>
        </div>
        {allUsers.filter(u => !members.find(m => m.user_id === u.id)).length > 0 && (
            <>
                <div className="ws-pick-label">Add a member</div>
                <div className="ws-pick">
                    {allUsers.filter(u => !members.find(m => m.user_id === u.id)).map(u => (
                        <div key={u.id} className="ws-pick__item" onClick={() => onAddMember(u.id)}>
                            <ServiceTile name={u.username || u.email || '?'} size={24} className="ws-row__av" />
                            <span className="ws-pick__name">{u.username || u.email}</span>
                            <Plus size={14} className="ws-pick__plus" />
                        </div>
                    ))}
                </div>
            </>
        )}
    </>
);

export default WorkspaceMembersTab;
