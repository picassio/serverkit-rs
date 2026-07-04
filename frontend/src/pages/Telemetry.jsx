import { useState, useEffect, useCallback } from 'react';
import { useSearchParams } from 'react-router-dom';
import {
    Activity,
    AlertTriangle,
    ChevronDown,
    ChevronRight,
    ChevronUp,
    Filter,
    Info,
    Loader2,
    RefreshCw,
    Search,
    Trash2,
    X,
    XCircle,
} from 'lucide-react';
import api from '../services/api';
import { PageTopbar } from '@/components/ds';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useAuth } from '../contexts/AuthContext';
import { useToast } from '../contexts/ToastContext';

const SEVERITY_ORDER = ['critical', 'error', 'warning', 'info', 'debug'];

const SEVERITY_CONFIG = {
    critical: { icon: XCircle, color: '#fb6f6f', label: 'Critical' },
    error: { icon: XCircle, color: '#ff7b7b', label: 'Error' },
    warning: { icon: AlertTriangle, color: '#f5b945', label: 'Warning' },
    info: { icon: Info, color: '#6d7cff', label: 'Info' },
    debug: { icon: Activity, color: '#9aa1b2', label: 'Debug' },
};

const PAGE_SIZE = 50;

export default function Telemetry() {
    const [searchParams, setSearchParams] = useSearchParams();
    const { isAdmin } = useAuth();
    const { showToast } = useToast();

    const [events, setEvents] = useState([]);
    const [loading, setLoading] = useState(true);
    const [hasMore, setHasMore] = useState(false);
    const [page, setPage] = useState(1);
    const [stats, setStats] = useState(null);
    const [sources, setSources] = useState([]);
    const [eventTypes, setEventTypes] = useState([]);
    const [selectedEvent, setSelectedEvent] = useState(null);
    const [filtersExpanded, setFiltersExpanded] = useState(false);

    const [filters, setFilters] = useState({
        source: searchParams.get('source') || '',
        event_type: searchParams.get('event_type') || '',
        severity: searchParams.get('severity') || '',
        resource_type: searchParams.get('resource_type') || '',
        resource_id: searchParams.get('resource_id') || '',
        correlation_id: searchParams.get('correlation_id') || '',
        q: searchParams.get('q') || '',
        start_date: searchParams.get('start_date') || '',
        end_date: searchParams.get('end_date') || '',
    });

    const loadStats = useCallback(async () => {
        try {
            const data = await api.getTelemetryStats({ hours: 24 });
            setStats(data);
        } catch {
            // stats are optional
        }
    }, []);

    const loadFilters = useCallback(async () => {
        try {
            const [sourcesData, typesData] = await Promise.all([
                api.getTelemetrySources(),
                api.getTelemetryEventTypes({ source: filters.source || undefined }),
            ]);
            setSources(sourcesData.sources || []);
            setEventTypes(typesData.event_types || []);
        } catch {
            // filters are optional
        }
    }, [filters.source]);

    const fetchEvents = useCallback(async (nextPage = 1, replace = true) => {
        setLoading(true);
        try {
            const params = { ...filters, per_page: PAGE_SIZE, page: nextPage };
            Object.keys(params).forEach((key) => {
                if (params[key] === '') delete params[key];
            });
            const data = await api.getTelemetryEvents(params);
            const fresh = data.events || [];
            setEvents((prev) => (replace ? fresh : [...prev, ...fresh]));
            setHasMore(fresh.length === PAGE_SIZE);
            setPage(nextPage);
        } catch (err) {
            showToast(`Failed to load telemetry: ${err.message}`, 'error');
            setHasMore(false);
        } finally {
            setLoading(false);
        }
    }, [filters, showToast]);

    useEffect(() => {
        loadStats();
        loadFilters();
        fetchEvents(1, true);
    }, [loadStats, loadFilters, fetchEvents]);

    const applyFilters = () => {
        const next = new URLSearchParams();
        Object.entries(filters).forEach(([key, value]) => {
            if (value) next.set(key, value);
        });
        setSearchParams(next);
        fetchEvents(1, true);
    };

    const clearFilters = () => {
        const empty = {
            source: '',
            event_type: '',
            severity: '',
            resource_type: '',
            resource_id: '',
            correlation_id: '',
            q: '',
            start_date: '',
            end_date: '',
        };
        setFilters(empty);
        setSearchParams(new URLSearchParams());
        setFiltersExpanded(false);
        fetchEvents(1, true);
    };

    const loadMore = () => {
        if (!loading && hasMore) {
            fetchEvents(page + 1, false);
        }
    };

    const handleFilterChange = (key, value) => {
        setFilters((prev) => ({ ...prev, [key]: value }));
    };

    const handleCorrelationClick = (correlationId) => {
        if (!correlationId) return;
        setFilters((prev) => ({ ...prev, correlation_id: correlationId }));
        setSearchParams({ correlation_id: correlationId });
        fetchEvents(1, true);
    };

    const emitTestEvent = async () => {
        try {
            await api.emitTestTelemetryEvent({
                source: 'system',
                event_type: 'telemetry.test',
                message: 'Test event from UI',
                severity: 'info',
                payload: { from_ui: true },
            });
            showToast('Test event emitted', 'success');
            fetchEvents(1, true);
            loadStats();
        } catch (err) {
            showToast(`Failed to emit test event: ${err.message}`, 'error');
        }
    };

    const cleanupOldEvents = async () => {
        if (!window.confirm('Delete telemetry events older than 90 days? This cannot be undone.')) {
            return;
        }
        try {
            const data = await api.cleanupTelemetryEvents(90);
            showToast(`Deleted ${data.deleted} old events`, 'success');
            fetchEvents(1, true);
            loadStats();
        } catch (err) {
            showToast(`Cleanup failed: ${err.message}`, 'error');
        }
    };

    const activeFilterCount = Object.values(filters).filter(Boolean).length;

    return (
        <>
            <PageTopbar
                icon={<Activity size={18} />}
                title="Telemetry"
                meta={stats ? `${stats.total || 0} events in last 24h` : 'System event stream'}
                actions={(
                    <>
                        {isAdmin && (
                            <Button variant="outline" size="sm" onClick={emitTestEvent}>
                                <Activity size={15} /> Test event
                            </Button>
                        )}
                        {isAdmin && (
                            <Button variant="outline" size="sm" onClick={cleanupOldEvents}>
                                <Trash2 size={15} /> Cleanup
                            </Button>
                        )}
                        <Button variant="outline" size="sm" onClick={() => fetchEvents(1, true)} disabled={loading}>
                            <RefreshCw size={15} className={loading ? 'spin' : ''} /> Refresh
                        </Button>
                    </>
                )}
            />

            <div className="telemetry-page">
                {stats && (
                    <div className="telemetry-stats">
                        {SEVERITY_ORDER.map((severity) => (
                            <div key={severity} className={`telemetry-stat telemetry-stat--${severity}`}>
                                <span className="telemetry-stat__count">
                                    {stats.by_severity?.[severity] || 0}
                                </span>
                                <span className="telemetry-stat__label">
                                    {SEVERITY_CONFIG[severity]?.label}
                                </span>
                            </div>
                        ))}
                        <div className="telemetry-stat telemetry-stat--total">
                            <span className="telemetry-stat__count">{stats.total || 0}</span>
                            <span className="telemetry-stat__label">Total</span>
                        </div>
                    </div>
                )}

                <div className="telemetry-filters">
                    <button
                        type="button"
                        className="telemetry-filters__toggle"
                        onClick={() => setFiltersExpanded((v) => !v)}
                    >
                        <Filter size={15} />
                        Filters
                        {activeFilterCount > 0 && (
                            <span className="telemetry-filters__count">{activeFilterCount}</span>
                        )}
                        {filtersExpanded ? <ChevronUp size={15} /> : <ChevronDown size={15} />}
                    </button>

                    {filtersExpanded && (
                        <div className="telemetry-filters__body">
                            <div className="telemetry-filters__grid">
                                <select
                                    value={filters.source}
                                    onChange={(e) => handleFilterChange('source', e.target.value)}
                                >
                                    <option value="">All sources</option>
                                    {sources.map((s) => (
                                        <option key={s} value={s}>{s}</option>
                                    ))}
                                </select>
                                <select
                                    value={filters.event_type}
                                    onChange={(e) => handleFilterChange('event_type', e.target.value)}
                                >
                                    <option value="">All event types</option>
                                    {eventTypes.map((t) => (
                                        <option key={t} value={t}>{t}</option>
                                    ))}
                                </select>
                                <select
                                    value={filters.severity}
                                    onChange={(e) => handleFilterChange('severity', e.target.value)}
                                >
                                    <option value="">All severities</option>
                                    {SEVERITY_ORDER.map((s) => (
                                        <option key={s} value={s}>{SEVERITY_CONFIG[s].label}</option>
                                    ))}
                                </select>
                                <Input
                                    placeholder="Resource type"
                                    value={filters.resource_type}
                                    onChange={(e) => handleFilterChange('resource_type', e.target.value)}
                                />
                                <Input
                                    placeholder="Resource ID"
                                    value={filters.resource_id}
                                    onChange={(e) => handleFilterChange('resource_id', e.target.value)}
                                />
                                <Input
                                    placeholder="Correlation ID"
                                    value={filters.correlation_id}
                                    onChange={(e) => handleFilterChange('correlation_id', e.target.value)}
                                />
                                <Input
                                    type="datetime-local"
                                    value={filters.start_date}
                                    onChange={(e) => handleFilterChange('start_date', e.target.value)}
                                />
                                <Input
                                    type="datetime-local"
                                    value={filters.end_date}
                                    onChange={(e) => handleFilterChange('end_date', e.target.value)}
                                />
                            </div>
                            <div className="telemetry-filters__search">
                                <Search size={15} />
                                <Input
                                    placeholder="Search message..."
                                    value={filters.q}
                                    onChange={(e) => handleFilterChange('q', e.target.value)}
                                />
                            </div>
                            <div className="telemetry-filters__actions">
                                <Button size="sm" onClick={applyFilters}>Apply</Button>
                                <Button size="sm" variant="ghost" onClick={clearFilters}>
                                    <X size={15} /> Clear
                                </Button>
                            </div>
                        </div>
                    )}
                </div>

                <div className="telemetry-list">
                    {events.map((event) => {
                        const config = SEVERITY_CONFIG[event.severity] || SEVERITY_CONFIG.info;
                        const Icon = config.icon;
                        return (
                            <div
                                key={event.id}
                                className={`telemetry-item telemetry-item--${event.severity}`}
                                onClick={() => setSelectedEvent(event)}
                            >
                                <div className="telemetry-item__severity">
                                    <Icon size={16} color={config.color} />
                                </div>
                                <div className="telemetry-item__content">
                                    <div className="telemetry-item__header">
                                        <span className="telemetry-item__source">{event.source}</span>
                                        <span className="telemetry-item__type">{event.event_type}</span>
                                        <span className="telemetry-item__time">
                                            {new Date(event.timestamp).toLocaleString()}
                                        </span>
                                    </div>
                                    <div className="telemetry-item__message">{event.message || event.event_type}</div>
                                    <div className="telemetry-item__meta">
                                        {event.resource_type && (
                                            <span className="telemetry-item__badge">
                                                {event.resource_type}:{event.resource_id}
                                            </span>
                                        )}
                                        {event.actor_username && (
                                            <span className="telemetry-item__badge">
                                                by {event.actor_username}
                                            </span>
                                        )}
                                        {event.correlation_id && (
                                            <button
                                                type="button"
                                                className="telemetry-item__correlation"
                                                onClick={(e) => {
                                                    e.stopPropagation();
                                                    handleCorrelationClick(event.correlation_id);
                                                }}
                                                title="View related events"
                                            >
                                                related <ChevronRight size={12} />
                                            </button>
                                        )}
                                    </div>
                                </div>
                            </div>
                        );
                    })}

                    {events.length === 0 && !loading && (
                        <div className="telemetry-empty">
                            <Info size={32} />
                            <p>No telemetry events match your filters.</p>
                        </div>
                    )}

                    {loading && (
                        <div className="telemetry-loading">
                            <Loader2 size={24} className="spin" />
                        </div>
                    )}

                    {hasMore && !loading && (
                        <Button variant="outline" className="telemetry-load-more" onClick={loadMore}>
                            Load more
                        </Button>
                    )}
                </div>
            </div>

            {selectedEvent && (
                <div className="telemetry-drawer-backdrop" onClick={() => setSelectedEvent(null)}>
                    <div className="telemetry-drawer" onClick={(e) => e.stopPropagation()}>
                        <div className="telemetry-drawer__header">
                            <h3>Event Details</h3>
                            <Button variant="ghost" size="sm" onClick={() => setSelectedEvent(null)}>
                                <X size={16} />
                            </Button>
                        </div>
                        <div className="telemetry-drawer__body">
                            <div className="telemetry-detail__row">
                                <span>ID</span>
                                <code>{selectedEvent.id}</code>
                            </div>
                            <div className="telemetry-detail__row">
                                <span>Timestamp</span>
                                <span>{new Date(selectedEvent.timestamp).toLocaleString()}</span>
                            </div>
                            <div className="telemetry-detail__row">
                                <span>Source</span>
                                <span>{selectedEvent.source}</span>
                            </div>
                            <div className="telemetry-detail__row">
                                <span>Type</span>
                                <span>{selectedEvent.event_type}</span>
                            </div>
                            <div className="telemetry-detail__row">
                                <span>Severity</span>
                                <span className={`telemetry-detail__severity telemetry-detail__severity--${selectedEvent.severity}`}>
                                    {selectedEvent.severity}
                                </span>
                            </div>
                            <div className="telemetry-detail__row">
                                <span>Message</span>
                                <span>{selectedEvent.message || '-'}</span>
                            </div>
                            {selectedEvent.resource_type && (
                                <div className="telemetry-detail__row">
                                    <span>Resource</span>
                                    <span>{selectedEvent.resource_type}:{selectedEvent.resource_id}</span>
                                </div>
                            )}
                            {selectedEvent.actor_username && (
                                <div className="telemetry-detail__row">
                                    <span>Actor</span>
                                    <span>{selectedEvent.actor_username}</span>
                                </div>
                            )}
                            {selectedEvent.correlation_id && (
                                <div className="telemetry-detail__row">
                                    <span>Correlation</span>
                                    <button
                                        type="button"
                                        className="telemetry-item__correlation"
                                        onClick={() => handleCorrelationClick(selectedEvent.correlation_id)}
                                    >
                                        {selectedEvent.correlation_id} <ChevronRight size={12} />
                                    </button>
                                </div>
                            )}
                            <div className="telemetry-detail__payload">
                                <span>Payload</span>
                                <pre>{JSON.stringify(selectedEvent.payload || {}, null, 2)}</pre>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </>
    );
}
