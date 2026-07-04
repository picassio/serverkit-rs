import { ServiceTile } from '@/components/ds';

const WorkspaceOverviewTab = ({ ws, since, members, srvIn, services, sites }) => (
    <div className="ws-detail__grid">
        <section className="ws-detail__card">
            <h3>Workspace</h3>
            <div className="sk-info-row"><span className="k">Slug</span><span className="v">/{ws.slug}</span></div>
            <div className="sk-info-row"><span className="k">Created</span><span className="v">{since || '—'}</span></div>
            <div className="sk-info-row"><span className="k">Max servers</span><span className="v">{ws.max_servers > 0 ? ws.max_servers : 'Unlimited'}</span></div>
            <div className="sk-info-row"><span className="k">Max users</span><span className="v">{ws.max_users > 0 ? ws.max_users : 'Unlimited'}</span></div>
        </section>
        <section className="ws-detail__card">
            <h3>Resources</h3>
            <div className="ws-resource-stats">
                <div className="ws-resource-stats__item">
                    <div className="ws-resource-stats__value">{srvIn.length}</div>
                    <div className="ws-resource-stats__label">Servers</div>
                </div>
                <div className="ws-resource-stats__item">
                    <div className="ws-resource-stats__value">{services.length}</div>
                    <div className="ws-resource-stats__label">Services</div>
                </div>
                <div className="ws-resource-stats__item">
                    <div className="ws-resource-stats__value">{sites.length}</div>
                    <div className="ws-resource-stats__label">Sites</div>
                </div>
                <div className="ws-resource-stats__item">
                    <div className="ws-resource-stats__value">{members.length}</div>
                    <div className="ws-resource-stats__label">Members</div>
                </div>
            </div>
        </section>
    </div>
);

export default WorkspaceOverviewTab;
