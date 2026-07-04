import { useState, useEffect, useCallback, useRef } from 'react';
import {
    Layers, RefreshCw, Inbox, Play, X, RotateCcw, ChevronRight, Clock,
} from 'lucide-react';
import api from '../services/api';
import { PageTopbar, MetricCard, Pill } from '@/components/ds';
import { Button } from '@/components/ui/button';
import { useAuth } from '../contexts/AuthContext';
import { useToast } from '../contexts/ToastContext';
import { timeAgo } from '../utils/timeAgo';

const STATUSES = ['all', 'pending', 'running', 'succeeded', 'failed', 'cancelled'];
const POLL_MS = 5000;

// Job status -> Pill colour.
const PILL_KIND = {
    succeeded: 'green',
    running: 'cyan',
    pending: 'amber',
    failed: 'red',
    cancelled: 'gray',
};

function StatusPill({ status }) {
    return <Pill kind={PILL_KIND[status] || 'gray'}>{status}</Pill>;
}

function describeSchedule(s) {
    if (s.schedule_kind === 'cron' && s.cron) return s.cron;
    const sec = s.interval_seconds;
    if (!sec) return '—';
    if (sec % 86400 === 0) return `every ${sec / 86400}d`;
    if (sec % 3600 === 0) return `every ${sec / 3600}h`;
    if (sec % 60 === 0) return `every ${sec / 60}m`;
    return `every ${sec}s`;
}

function JobDetail({ detail }) {
    if (!detail) return <div className="jobs-detail jobs-detail--loading">Loading details…</div>;
    const payload = detail.payload && Object.keys(detail.payload).length > 0 ? detail.payload : null;
    return (
        <div className="jobs-detail">
            <div className="jobs-detail__grid">
                <span>ID</span><code>{detail.id}</code>
                <span>Correlation</span><code>{detail.correlation_id || '—'}</code>
                <span>Started</span><span>{detail.started_at ? new Date(detail.started_at).toLocaleString() : '—'}</span>
                <span>Completed</span><span>{detail.completed_at ? new Date(detail.completed_at).toLocaleString() : '—'}</span>
                <span>Duration</span><span>{detail.duration != null ? `${detail.duration.toFixed(2)}s` : '—'}</span>
            </div>
            {detail.error_message && (
                <div className="jobs-detail__block jobs-detail__block--error">
                    <span>Error</span>
                    <pre>{detail.error_message}</pre>
                </div>
            )}
            {payload && (
                <div className="jobs-detail__block">
                    <span>Payload</span>
                    <pre>{JSON.stringify(payload, null, 2)}</pre>
                </div>
            )}
            {detail.result != null && (
                <div className="jobs-detail__block">
                    <span>Result</span>
                    <pre>{JSON.stringify(detail.result, null, 2)}</pre>
                </div>
            )}
        </div>
    );
}

function JobRow({ job, expanded, detail, onToggle, onCancel, onRetry }) {
    const canCancel = job.status === 'pending' || job.status === 'running';
    const canRetry = job.status === 'failed' || job.status === 'cancelled';
    return (
        <>
            <tr className={`jobs-row${expanded ? ' is-open' : ''}`} onClick={onToggle}>
                <td className="jobs-caret">
                    <ChevronRight size={14} className={expanded ? 'is-open' : ''} />
                </td>
                <td><StatusPill status={job.status} /></td>
                <td className="jobs-mono">{job.kind}</td>
                <td className="jobs-owner">
                    {job.owner_type ? `${job.owner_type}${job.owner_id ? `:${job.owner_id}` : ''}` : '—'}
                </td>
                <td>{job.attempts}/{job.max_attempts}</td>
                <td className="jobs-when">{timeAgo(job.created_at)}</td>
                <td className="jobs-actions" onClick={(e) => e.stopPropagation()}>
                    {canCancel && (
                        <Button variant="ghost" size="sm" onClick={onCancel}><X size={14} /> Cancel</Button>
                    )}
                    {canRetry && (
                        <Button variant="ghost" size="sm" onClick={onRetry}><RotateCcw size={14} /> Retry</Button>
                    )}
                </td>
            </tr>
            {expanded && (
                <tr className="jobs-detail-row">
                    <td colSpan={7}><JobDetail detail={detail} /></td>
                </tr>
            )}
        </>
    );
}

export default function Jobs() {
    const { isAdmin } = useAuth();
    const toast = useToast();
    const [jobs, setJobs] = useState([]);
    const [stats, setStats] = useState(null);
    const [kinds, setKinds] = useState([]);
    const [scheduled, setScheduled] = useState([]);
    const [status, setStatus] = useState('all');
    const [kind, setKind] = useState('all');
    const [expandedId, setExpandedId] = useState(null);
    const [detail, setDetail] = useState(null);
    const [loading, setLoading] = useState(true);
    const pollRef = useRef(null);

    const load = useCallback(async () => {
        try {
            const params = {};
            if (status !== 'all') params.status = status;
            if (kind !== 'all') params.kind = kind;
            const [jobsData, statsData, schedData] = await Promise.all([
                api.getJobs(params),
                api.getJobStats(),
                api.getScheduledJobs(),
            ]);
            setJobs(jobsData.jobs || []);
            setStats(statsData || null);
            setScheduled(schedData.scheduled || []);
        } catch {
            // leave the last good state on screen
        } finally {
            setLoading(false);
        }
    }, [status, kind]);

    useEffect(() => {
        if (!isAdmin) return undefined;
        load();
        pollRef.current = setInterval(load, POLL_MS);
        return () => clearInterval(pollRef.current);
    }, [isAdmin, load]);

    useEffect(() => {
        if (!isAdmin) return;
        api.getJobKinds().then((d) => setKinds(d.kinds || [])).catch(() => {});
    }, [isAdmin]);

    const toggleDetail = async (job) => {
        if (expandedId === job.id) {
            setExpandedId(null);
            setDetail(null);
            return;
        }
        setExpandedId(job.id);
        setDetail(null);
        try {
            const data = await api.getJob(job.id);
            setDetail(data.job || null);
        } catch {
            setDetail(null);
        }
    };

    const action = async (fn, okMsg, failMsg) => {
        try {
            await fn();
            toast.success(okMsg);
            load();
        } catch (e) {
            toast.error(e?.message || failMsg);
        }
    };

    if (!isAdmin) {
        return (
            <>
                <PageTopbar icon={<Layers size={18} />} title="Jobs" />
                <div className="jobs-page"><div className="jobs-empty">Admins only.</div></div>
            </>
        );
    }

    const byStatus = stats?.by_status || {};

    return (
        <>
            <PageTopbar
                icon={<Layers size={18} />}
                title="Jobs"
                meta="Background work across the Queue Bus"
                actions={(
                    <Button variant="outline" size="sm" onClick={load}>
                        <RefreshCw size={14} /> Refresh
                    </Button>
                )}
            />

            <div className="jobs-page">
                <div className="jobs-stats">
                    <MetricCard label="Total" value={stats?.total ?? 0} tone="accent" />
                    <MetricCard label="Running" value={byStatus.running ?? 0} />
                    <MetricCard label="Pending" value={byStatus.pending ?? 0} tone="amber" />
                    <MetricCard label="Succeeded" value={byStatus.succeeded ?? 0} tone="green" />
                    <MetricCard label="Failed" value={byStatus.failed ?? 0} tone="red" />
                </div>

                <div className="jobs-filters">
                    <label>
                        Status
                        <select value={status} onChange={(e) => setStatus(e.target.value)}>
                            {STATUSES.map((s) => <option key={s} value={s}>{s}</option>)}
                        </select>
                    </label>
                    <label>
                        Kind
                        <select value={kind} onChange={(e) => setKind(e.target.value)}>
                            <option value="all">all</option>
                            {kinds.map((k) => <option key={k} value={k}>{k}</option>)}
                        </select>
                    </label>
                </div>

                {loading && jobs.length === 0 ? (
                    <div className="jobs-empty">Loading…</div>
                ) : jobs.length === 0 ? (
                    <div className="jobs-empty">
                        <Inbox size={24} aria-hidden="true" />
                        <p>No jobs match these filters.</p>
                    </div>
                ) : (
                    <div className="jobs-table-wrap">
                        <table className="jobs-table">
                            <thead>
                                <tr>
                                    <th aria-label="expand" />
                                    <th>Status</th>
                                    <th>Kind</th>
                                    <th>Owner</th>
                                    <th>Tries</th>
                                    <th>Created</th>
                                    <th aria-label="Actions" />
                                </tr>
                            </thead>
                            <tbody>
                                {jobs.map((j) => (
                                    <JobRow
                                        key={j.id}
                                        job={j}
                                        expanded={expandedId === j.id}
                                        detail={expandedId === j.id ? detail : null}
                                        onToggle={() => toggleDetail(j)}
                                        onCancel={() => action(() => api.cancelJob(j.id), 'Job cancelled', 'Cancel failed')}
                                        onRetry={() => action(() => api.retryJob(j.id), 'Job re-queued', 'Retry failed')}
                                    />
                                ))}
                            </tbody>
                        </table>
                    </div>
                )}

                <div className="jobs-section-head">
                    <Clock size={16} aria-hidden="true" />
                    <h2>Scheduled jobs</h2>
                </div>

                {scheduled.length === 0 ? (
                    <div className="jobs-empty"><p>No scheduled jobs.</p></div>
                ) : (
                    <div className="jobs-table-wrap">
                        <table className="jobs-table">
                            <thead>
                                <tr>
                                    <th>Name</th>
                                    <th>Kind</th>
                                    <th>Schedule</th>
                                    <th>Next run</th>
                                    <th>Last run</th>
                                    <th aria-label="Actions" />
                                </tr>
                            </thead>
                            <tbody>
                                {scheduled.map((s) => (
                                    <tr key={s.id} className={s.enabled ? '' : 'is-disabled'}>
                                        <td className="jobs-mono">{s.name}</td>
                                        <td className="jobs-mono">{s.kind}</td>
                                        <td>{describeSchedule(s)}</td>
                                        <td>
                                            {s.enabled
                                                ? (s.next_run_at ? timeAgo(s.next_run_at) : '—')
                                                : <Pill kind="gray">paused</Pill>}
                                        </td>
                                        <td className="jobs-when">{s.last_run_at ? timeAgo(s.last_run_at) : '—'}</td>
                                        <td className="jobs-actions">
                                            <Button variant="ghost" size="sm" title="Run now"
                                                onClick={() => action(() => api.runScheduledJob(s.id), `Enqueued "${s.name}"`, 'Run failed')}>
                                                <Play size={14} /> Run
                                            </Button>
                                            <Button variant="ghost" size="sm"
                                                onClick={() => action(
                                                    () => api.setScheduledJobEnabled(s.id, !s.enabled),
                                                    s.enabled ? `Paused "${s.name}"` : `Enabled "${s.name}"`,
                                                    'Update failed',
                                                )}>
                                                {s.enabled ? 'Pause' : 'Enable'}
                                            </Button>
                                        </td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                )}
            </div>
        </>
    );
}
