import { useState, useEffect, useMemo } from 'react';
import { useTopbarActions } from '@/hooks/useTopbarActions';
import {
    Activity,
    AlertTriangle,
    BarChart3,
    CheckCircle,
    ChevronDown,
    Download,
    Eye,
    Layers,
    RefreshCw,
    Search,
    Server,
    TrendingUp,
    XCircle,
    Zap
} from 'lucide-react';
import {
    LineChart, Line, AreaChart, Area, XAxis, YAxis,
    CartesianGrid, Tooltip, ResponsiveContainer, Legend
} from 'recharts';
import api from '../services/api';
import { useToast } from '../contexts/ToastContext';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Pill } from '@/components/ds';

const CHART_COLORS = [
    '#6366f1', '#ec4899', '#14b8a6', '#f59e0b',
    '#8b5cf6', '#06b6d4', '#ef4444', '#22c55e',
    '#a855f7', '#f97316'
];

const METRIC_LABELS = {
    cpu: 'CPU %',
    memory: 'Memory %',
    disk: 'Disk %',
    network_rx: 'Network RX',
    network_tx: 'Network TX'
};

const heatLevel = (value) => {
    if (value == null) return 'empty';
    if (value >= 90) return 'critical';
    if (value >= 75) return 'high';
    if (value >= 50) return 'medium';
    return 'low';
};

const heatColor = (value) => {
    const level = heatLevel(value);
    if (level === 'empty') return 'var(--card-bg)';
    if (level === 'critical') return 'var(--red)';
    if (level === 'high') return '#f97316';
    if (level === 'medium') return 'var(--amber)';
    return 'var(--green)';
};

const FleetMonitor = () => {
    const [activeTab, setActiveTab] = useState('overview');
    const [loading, setLoading] = useState(true);
    const toast = useToast();

    // Publish the Refresh button to the shared tab-group top bar; re-registers
    // on `loading` so the spinner/disabled state stays in sync.
    useTopbarActions(() =>
        <Button size="sm" onClick={fetchTabData} disabled={loading}>
            <RefreshCw size={18} className={loading ? 'animate-spin' : ''} />
            Refresh
        </Button>,
        [loading]
    );

    // Overview state
    const [heatmapData, setHeatmapData] = useState([]);

    // Comparison state
    const [servers, setServers] = useState([]);
    const [selectedServers, setSelectedServers] = useState([]);
    const [compMetric, setCompMetric] = useState('cpu');
    const [compPeriod, setCompPeriod] = useState('24h');
    const [compData, setCompData] = useState(null);

    // Alerts state
    const [alerts, setAlerts] = useState([]);
    const [thresholds, setThresholds] = useState([]);
    const [alertFilter, setAlertFilter] = useState('active');
    const [newThreshold, setNewThreshold] = useState({
        metric: 'cpu', warning_threshold: 80, critical_threshold: 95, duration_seconds: 300
    });

    // Anomaly/Forecast state
    const [anomalies, setAnomalies] = useState([]);
    const [forecastServer, setForecastServer] = useState('');
    const [forecastMetric, setForecastMetric] = useState('disk');
    const [forecast, setForecast] = useState(null);

    // Search state
    const [searchQuery, setSearchQuery] = useState('');
    const [searchType, setSearchType] = useState('any');
    const [searchResults, setSearchResults] = useState([]);

    useEffect(() => {
        loadServers();
    }, []);

    useEffect(() => {
        fetchTabData();
    }, [activeTab, alertFilter]);

    const loadServers = async () => {
        try {
            const data = await api.getServers();
            const list = data.servers || data;
            setServers(Array.isArray(list) ? list : []);
        } catch (e) {
            console.error('Error loading servers:', e);
        }
    };

    const fetchTabData = async () => {
        setLoading(true);
        try {
            if (activeTab === 'overview') {
                const data = await api.getFleetHeatmap();
                setHeatmapData(data);
            } else if (activeTab === 'alerts') {
                const [alertData, thresholdData] = await Promise.all([
                    api.getFleetAlerts({ status: alertFilter !== 'all' ? alertFilter : undefined }),
                    api.getFleetThresholds()
                ]);
                setAlerts(alertData);
                setThresholds(thresholdData);
            } else if (activeTab === 'anomalies') {
                const data = await api.getFleetAnomalies();
                setAnomalies(data);
            }
        } catch (e) {
            console.error('Error fetching tab data:', e);
        } finally {
            setLoading(false);
        }
    };

    const loadComparison = async () => {
        if (selectedServers.length === 0) return;
        setLoading(true);
        try {
            const data = await api.getFleetComparison(selectedServers, compMetric, compPeriod);
            setCompData(data);
        } catch (e) {
            toast.error('Failed to load comparison data');
        } finally {
            setLoading(false);
        }
    };

    const loadForecast = async (serverId) => {
        try {
            const data = await api.getCapacityForecast(serverId || forecastServer, forecastMetric);
            setForecast(data);
        } catch (e) {
            toast.error('Failed to load forecast');
        }
    };

    const handleSearch = async () => {
        if (searchQuery.length < 2) return;
        setLoading(true);
        try {
            const data = await api.searchFleet(searchQuery, searchType);
            setSearchResults(data);
        } catch (e) {
            toast.error('Search failed');
        } finally {
            setLoading(false);
        }
    };

    const acknowledgeAlert = async (id) => {
        try {
            await api.acknowledgeFleetAlert(id);
            fetchTabData();
        } catch (e) {
            toast.error('Failed to acknowledge alert');
        }
    };

    const resolveAlert = async (id) => {
        try {
            await api.resolveFleetAlert(id);
            fetchTabData();
        } catch (e) {
            toast.error('Failed to resolve alert');
        }
    };

    const saveThreshold = async () => {
        try {
            await api.createFleetThreshold(newThreshold);
            toast.success('Threshold saved');
            fetchTabData();
        } catch (e) {
            toast.error('Failed to save threshold');
        }
    };

    const deleteThreshold = async (id) => {
        try {
            await api.deleteFleetThreshold(id);
            fetchTabData();
        } catch (e) {
            toast.error('Failed to delete threshold');
        }
    };

    const toggleServer = (id) => {
        setSelectedServers(prev =>
            prev.includes(id) ? prev.filter(s => s !== id) : [...prev, id]
        );
    };

    // Merge comparison series into recharts-compatible format
    const compChartData = useMemo(() => {
        if (!compData?.series?.length) return [];
        const timeMap = {};
        compData.series.forEach((s, idx) => {
            s.data.forEach(point => {
                if (!timeMap[point.timestamp]) {
                    timeMap[point.timestamp] = { timestamp: point.timestamp };
                }
                timeMap[point.timestamp][s.name] = point.value;
            });
        });
        return Object.values(timeMap).sort((a, b) => a.timestamp.localeCompare(b.timestamp));
    }, [compData]);

    const exportCsv = async () => {
        if (selectedServers.length === 0) return;
        try {
            const blob = await api.exportFleetCsv(selectedServers, compMetric, compPeriod);
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `fleet_${compMetric}_${compPeriod}.csv`;
            a.click();
            URL.revokeObjectURL(url);
        } catch (e) {
            toast.error('Export failed');
        }
    };

    return (
        <div className="sk-tabgroup__inner">
            <Tabs value={activeTab} onValueChange={setActiveTab}>
                <TabsList>
                    {[
                        { key: 'overview', icon: Layers, label: 'Overview' },
                        { key: 'comparison', icon: BarChart3, label: 'Comparison' },
                        { key: 'alerts', icon: AlertTriangle, label: 'Alerts' },
                        { key: 'anomalies', icon: TrendingUp, label: 'Anomalies & Forecast' },
                        { key: 'search', icon: Search, label: 'Fleet Search' },
                    ].map(tab => (
                        <TabsTrigger key={tab.key} value={tab.key}>
                            <tab.icon size={18} />
                            {tab.label}
                        </TabsTrigger>
                    ))}
                </TabsList>
            </Tabs>

            <div className="tab-content mt-6">
                {/* ==================== Overview Heatmap ==================== */}
                {activeTab === 'overview' && (
                    <div className="space-y-6">
                        <div className="card">
                            <div className="card-header">
                                <h2>Server Health Heatmap</h2>
                            </div>
                            <div className="card-body">
                                {heatmapData.length > 0 ? (
                                    <div className="fleet-heatmap">
                                        <div className="fleet-heatmap__header">
                                            <div className="fleet-heatmap__label">Server</div>
                                            <div className="fleet-heatmap__label">CPU</div>
                                            <div className="fleet-heatmap__label">Memory</div>
                                            <div className="fleet-heatmap__label">Disk</div>
                                            <div className="fleet-heatmap__label">Containers</div>
                                            <div className="fleet-heatmap__label">Status</div>
                                        </div>
                                        {heatmapData.map(server => (
                                            <div key={server.id} className="fleet-heatmap__row">
                                                <div className="fleet-heatmap__server">
                                                    <Server size={14} />
                                                    <span>{server.name}</span>
                                                    {server.group_name && (
                                                        <span className="text-xs text-gray-400 ml-1">({server.group_name})</span>
                                                    )}
                                                </div>
                                                <div
                                                    className={`fleet-heatmap__cell is-${heatLevel(server.cpu)}`}
                                                    title={`CPU: ${server.cpu ?? 'N/A'}%`}
                                                >
                                                    {server.cpu != null ? `${server.cpu}%` : '-'}
                                                </div>
                                                <div
                                                    className={`fleet-heatmap__cell is-${heatLevel(server.memory)}`}
                                                    title={`Memory: ${server.memory ?? 'N/A'}%`}
                                                >
                                                    {server.memory != null ? `${server.memory}%` : '-'}
                                                </div>
                                                <div
                                                    className={`fleet-heatmap__cell is-${heatLevel(server.disk)}`}
                                                    title={`Disk: ${server.disk ?? 'N/A'}%`}
                                                >
                                                    {server.disk != null ? `${server.disk}%` : '-'}
                                                </div>
                                                <div className="fleet-heatmap__cell">
                                                    {server.containers ?? '-'}
                                                </div>
                                                <div className="fleet-heatmap__cell">
                                                    <Pill kind={server.status === 'online' ? 'green' : 'red'}>{server.status}</Pill>
                                                </div>
                                            </div>
                                        ))}
                                    </div>
                                ) : (
                                    <div className="py-12 text-center text-gray-500">
                                        <Server size={48} className="mx-auto text-gray-300 mb-4" />
                                        <p>No servers registered. Add servers to see the fleet heatmap.</p>
                                    </div>
                                )}
                            </div>
                        </div>
                        <div className="flex gap-3 items-center text-sm text-gray-500">
                            <span>Legend:</span>
                            <span className="flex items-center gap-1"><span className="fleet-heatmap__legend-dot is-low"></span> 0-50%</span>
                            <span className="flex items-center gap-1"><span className="fleet-heatmap__legend-dot is-medium"></span> 50-75%</span>
                            <span className="flex items-center gap-1"><span className="fleet-heatmap__legend-dot is-high"></span> 75-90%</span>
                            <span className="flex items-center gap-1"><span className="fleet-heatmap__legend-dot is-critical"></span> 90-100%</span>
                        </div>
                    </div>
                )}

                {/* ==================== Comparison ==================== */}
                {activeTab === 'comparison' && (
                    <div className="space-y-6">
                        <div className="card">
                            <div className="card-header flex justify-between items-center">
                                <h2>Server Comparison</h2>
                                <div className="flex gap-2">
                                    <Button variant="ghost" size="sm" onClick={exportCsv} disabled={selectedServers.length === 0}>
                                        <Download size={14} /> Export CSV
                                    </Button>
                                </div>
                            </div>
                            <div className="card-body">
                                <div className="grid grid-cols-1 lg:grid-cols-4 gap-4 mb-6">
                                    <div className="lg:col-span-2">
                                        <label className="text-sm font-medium mb-1 block">Select Servers (click to toggle)</label>
                                        <div className="flex flex-wrap gap-2 max-h-24 overflow-y-auto p-2 border rounded">
                                            {servers.map(s => (
                                                <Button
                                                    key={s.id}
                                                    size="sm"
                                                    variant={selectedServers.includes(s.id) ? 'default' : 'ghost'}
                                                    onClick={() => toggleServer(s.id)}
                                                >
                                                    {s.name}
                                                </Button>
                                            ))}
                                        </div>
                                    </div>
                                    <div>
                                        <label className="text-sm font-medium mb-1 block">Metric</label>
                                        <select className="form-select w-full" value={compMetric} onChange={e => setCompMetric(e.target.value)}>
                                            {Object.entries(METRIC_LABELS).map(([k, v]) => (
                                                <option key={k} value={k}>{v}</option>
                                            ))}
                                        </select>
                                    </div>
                                    <div>
                                        <label className="text-sm font-medium mb-1 block">Period</label>
                                        <div className="flex gap-1">
                                            {['1h', '6h', '24h', '7d'].map(p => (
                                                <Button
                                                    key={p}
                                                    size="sm"
                                                    variant={compPeriod === p ? 'default' : 'ghost'}
                                                    onClick={() => setCompPeriod(p)}
                                                >
                                                    {p}
                                                </Button>
                                            ))}
                                        </div>
                                    </div>
                                </div>
                                <div className="flex justify-end mb-4">
                                    <Button
                                        onClick={loadComparison}
                                        disabled={selectedServers.length === 0 || loading}
                                    >
                                        <BarChart3 size={16} /> Compare
                                    </Button>
                                </div>

                                {compChartData.length > 0 && compData?.series && (
                                    <ResponsiveContainer width="100%" height={400}>
                                        <AreaChart data={compChartData}>
                                            <CartesianGrid strokeDasharray="3 3" stroke="var(--border-color)" />
                                            <XAxis
                                                dataKey="timestamp"
                                                tickFormatter={v => new Date(v).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                                                stroke="var(--text-secondary)"
                                                fontSize={12}
                                            />
                                            <YAxis stroke="var(--text-secondary)" fontSize={12} unit="%" />
                                            <Tooltip
                                                labelFormatter={v => new Date(v).toLocaleString()}
                                                contentStyle={{ backgroundColor: 'var(--card-bg)', border: '1px solid var(--border-color)' }}
                                            />
                                            <Legend />
                                            {compData.series.map((s, i) => (
                                                <Area
                                                    key={s.server_id}
                                                    type="monotone"
                                                    dataKey={s.name}
                                                    stroke={CHART_COLORS[i % CHART_COLORS.length]}
                                                    fill={CHART_COLORS[i % CHART_COLORS.length]}
                                                    fillOpacity={0.1}
                                                    strokeWidth={2}
                                                    dot={false}
                                                />
                                            ))}
                                        </AreaChart>
                                    </ResponsiveContainer>
                                )}

                                {selectedServers.length === 0 && (
                                    <div className="py-12 text-center text-gray-500">
                                        <BarChart3 size={48} className="mx-auto text-gray-300 mb-4" />
                                        <p>Select servers above to compare their metrics.</p>
                                    </div>
                                )}
                            </div>
                        </div>
                    </div>
                )}

                {/* ==================== Alerts ==================== */}
                {activeTab === 'alerts' && (
                    <div className="space-y-6">
                        <div className="card">
                            <div className="card-header flex justify-between items-center">
                                <h2>Metric Alerts</h2>
                                <div className="flex gap-1">
                                    {['active', 'acknowledged', 'resolved', 'all'].map(f => (
                                        <Button
                                            key={f}
                                            size="sm"
                                            variant={alertFilter === f ? 'default' : 'ghost'}
                                            onClick={() => setAlertFilter(f)}
                                        >
                                            {f.charAt(0).toUpperCase() + f.slice(1)}
                                        </Button>
                                    ))}
                                </div>
                            </div>
                            {alerts.length > 0 ? (
                                <div className="overflow-x-auto">
                                    <table className="sk-dtable">
                                        <thead>
                                            <tr>
                                                <th>Server</th>
                                                <th>Metric</th>
                                                <th>Value</th>
                                                <th>Threshold</th>
                                                <th>Severity</th>
                                                <th>Status</th>
                                                <th>Time</th>
                                                <th>Actions</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {alerts.map(a => (
                                                <tr key={a.id}>
                                                    <td className="font-semibold">{a.server_name}</td>
                                                    <td>{METRIC_LABELS[a.metric] || a.metric}</td>
                                                    <td className="font-mono">{a.value}%</td>
                                                    <td className="font-mono">{a.threshold}%</td>
                                                    <td>
                                                        <Pill kind={a.severity === 'critical' ? 'red' : 'amber'}>{a.severity}</Pill>
                                                    </td>
                                                    <td>
                                                        <Pill kind={a.status === 'active' ? 'red' : a.status === 'acknowledged' ? 'amber' : 'green'}>
                                                            {a.status}
                                                        </Pill>
                                                    </td>
                                                    <td className="text-sm">{new Date(a.created_at).toLocaleString()}</td>
                                                    <td className="actions">
                                                        {a.status === 'active' && (
                                                            <>
                                                                <Button variant="ghost" size="sm" onClick={() => acknowledgeAlert(a.id)}>
                                                                    <Eye size={14} /> Ack
                                                                </Button>
                                                                <Button variant="ghost" size="sm" onClick={() => resolveAlert(a.id)}>
                                                                    <CheckCircle size={14} />
                                                                </Button>
                                                            </>
                                                        )}
                                                        {a.status === 'acknowledged' && (
                                                            <Button variant="ghost" size="sm" onClick={() => resolveAlert(a.id)}>
                                                                <CheckCircle size={14} /> Resolve
                                                            </Button>
                                                        )}
                                                    </td>
                                                </tr>
                                            ))}
                                        </tbody>
                                    </table>
                                </div>
                            ) : (
                                <div className="card-body py-8 text-center text-gray-500">
                                    <CheckCircle size={36} className="mx-auto text-gray-300 mb-3" />
                                    <p>No {alertFilter !== 'all' ? alertFilter : ''} alerts.</p>
                                </div>
                            )}
                        </div>

                        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                            <div className="card">
                                <div className="card-header"><h2>Alert Thresholds</h2></div>
                                {thresholds.length > 0 ? (
                                    <div className="overflow-x-auto">
                                        <table className="sk-dtable">
                                            <thead>
                                                <tr>
                                                    <th>Scope</th>
                                                    <th>Metric</th>
                                                    <th>Warning</th>
                                                    <th>Critical</th>
                                                    <th>Duration</th>
                                                    <th></th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {thresholds.map(t => (
                                                    <tr key={t.id}>
                                                        <td>{t.server_name || 'Global'}</td>
                                                        <td>{METRIC_LABELS[t.metric] || t.metric}</td>
                                                        <td className="text-yellow-600">{t.warning_threshold}%</td>
                                                        <td className="text-red-600">{t.critical_threshold}%</td>
                                                        <td>{t.duration_seconds}s</td>
                                                        <td>
                                                            <Button variant="ghost" size="sm" className="text-red-600" onClick={() => deleteThreshold(t.id)}>
                                                                <XCircle size={14} />
                                                            </Button>
                                                        </td>
                                                    </tr>
                                                ))}
                                            </tbody>
                                        </table>
                                    </div>
                                ) : (
                                    <div className="card-body py-6 text-center text-gray-500 text-sm">
                                        No thresholds configured. Add one to start receiving metric alerts.
                                    </div>
                                )}
                            </div>

                            <div className="card">
                                <div className="card-header"><h2>Add Threshold</h2></div>
                                <div className="card-body space-y-4">
                                    <div className="form-group">
                                        <label>Server (leave empty for global)</label>
                                        <select
                                            className="form-select w-full"
                                            value={newThreshold.server_id || ''}
                                            onChange={e => setNewThreshold(p => ({ ...p, server_id: e.target.value || undefined }))}
                                        >
                                            <option value="">Global Default</option>
                                            {servers.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}
                                        </select>
                                    </div>
                                    <div className="grid grid-cols-2 gap-4">
                                        <div className="form-group">
                                            <label>Metric</label>
                                            <select className="form-select w-full" value={newThreshold.metric}
                                                onChange={e => setNewThreshold(p => ({ ...p, metric: e.target.value }))}>
                                                <option value="cpu">CPU</option>
                                                <option value="memory">Memory</option>
                                                <option value="disk">Disk</option>
                                            </select>
                                        </div>
                                        <div className="form-group">
                                            <label>Duration (seconds)</label>
                                            <Input type="number" value={newThreshold.duration_seconds}
                                                onChange={e => setNewThreshold(p => ({ ...p, duration_seconds: parseInt(e.target.value) || 300 }))} />
                                        </div>
                                    </div>
                                    <div className="grid grid-cols-2 gap-4">
                                        <div className="form-group">
                                            <label>Warning (%)</label>
                                            <Input type="number" value={newThreshold.warning_threshold}
                                                onChange={e => setNewThreshold(p => ({ ...p, warning_threshold: parseFloat(e.target.value) || 80 }))} />
                                        </div>
                                        <div className="form-group">
                                            <label>Critical (%)</label>
                                            <Input type="number" value={newThreshold.critical_threshold}
                                                onChange={e => setNewThreshold(p => ({ ...p, critical_threshold: parseFloat(e.target.value) || 95 }))} />
                                        </div>
                                    </div>
                                    <Button className="w-full" onClick={saveThreshold}>
                                        Save Threshold
                                    </Button>
                                </div>
                            </div>
                        </div>
                    </div>
                )}

                {/* ==================== Anomalies & Forecast ==================== */}
                {activeTab === 'anomalies' && (
                    <div className="space-y-6">
                        <div className="card">
                            <div className="card-header"><h2>Anomaly Detection</h2></div>
                            {anomalies.length > 0 ? (
                                <div className="overflow-x-auto">
                                    <table className="sk-dtable">
                                        <thead>
                                            <tr>
                                                <th>Server</th>
                                                <th>Metric</th>
                                                <th>Current</th>
                                                <th>Baseline (mean)</th>
                                                <th>Std Dev</th>
                                                <th>Z-Score</th>
                                                <th>Direction</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {anomalies.map((a, i) => (
                                                <tr key={i}>
                                                    <td className="font-semibold">{a.server_name}</td>
                                                    <td>{METRIC_LABELS[a.metric] || a.metric}</td>
                                                    <td className="font-mono text-red-600">{a.current_value}%</td>
                                                    <td className="font-mono">{a.mean}%</td>
                                                    <td className="font-mono">{a.stddev}</td>
                                                    <td className="font-mono font-bold">{a.z_score}</td>
                                                    <td>
                                                        <Pill kind={a.direction === 'high' ? 'red' : 'cyan'}>
                                                            {a.direction === 'high' ? 'Unusually High' : 'Unusually Low'}
                                                        </Pill>
                                                    </td>
                                                </tr>
                                            ))}
                                        </tbody>
                                    </table>
                                </div>
                            ) : (
                                <div className="card-body py-8 text-center text-gray-500">
                                    <CheckCircle size={36} className="mx-auto text-gray-300 mb-3" />
                                    <p>No anomalies detected. All metrics are within normal ranges.</p>
                                </div>
                            )}
                        </div>

                        <div className="card">
                            <div className="card-header"><h2>Capacity Forecast</h2></div>
                            <div className="card-body">
                                <div className="flex gap-4 mb-6">
                                    <div className="form-group flex-1">
                                        <label>Server</label>
                                        <select className="form-select w-full" value={forecastServer}
                                            onChange={e => setForecastServer(e.target.value)}>
                                            <option value="">Select a server...</option>
                                            {servers.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}
                                        </select>
                                    </div>
                                    <div className="form-group">
                                        <label>Metric</label>
                                        <select className="form-select w-full" value={forecastMetric}
                                            onChange={e => setForecastMetric(e.target.value)}>
                                            <option value="disk">Disk</option>
                                            <option value="memory">Memory</option>
                                            <option value="cpu">CPU</option>
                                        </select>
                                    </div>
                                    <div className="form-group flex items-end">
                                        <Button onClick={() => loadForecast()} disabled={!forecastServer}>
                                            <TrendingUp size={16} /> Forecast
                                        </Button>
                                    </div>
                                </div>

                                {forecast && !forecast.error && (
                                    <div className="space-y-6">
                                        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                                            <div className="fleet-statbox p-4">
                                                <div className="text-sm text-gray-500">Current</div>
                                                <div className="text-2xl font-bold">{forecast.current_value}%</div>
                                            </div>
                                            <div className="fleet-statbox p-4">
                                                <div className="text-sm text-gray-500">Growth Rate</div>
                                                <div className="text-2xl font-bold">{forecast.growth_rate_per_day}%/day</div>
                                            </div>
                                            <div className="fleet-statbox p-4">
                                                <div className="text-sm text-gray-500">Trend</div>
                                                <div className={`text-2xl font-bold ${forecast.trend === 'increasing' ? 'text-red-600' : forecast.trend === 'decreasing' ? 'text-green-600' : ''}`}>
                                                    {forecast.trend}
                                                </div>
                                            </div>
                                        </div>

                                        {forecast.predictions && (
                                            <div className="grid grid-cols-2 gap-4">
                                                <div className={`fleet-predbox ${forecast.predictions.days_to_90pct === 0 ? 'is-red' : 'is-amber'}`}>
                                                    <div className="text-sm font-medium">Reaches 90%</div>
                                                    <div className="text-lg font-bold">
                                                        {forecast.predictions.date_90pct || 'N/A'}
                                                        {forecast.predictions.days_to_90pct != null && forecast.predictions.days_to_90pct > 0 && (
                                                            <span className="text-sm font-normal ml-2">({forecast.predictions.days_to_90pct} days)</span>
                                                        )}
                                                    </div>
                                                </div>
                                                <div className="fleet-predbox is-red">
                                                    <div className="text-sm font-medium">Reaches 100%</div>
                                                    <div className="text-lg font-bold">
                                                        {forecast.predictions.date_100pct || 'N/A'}
                                                        {forecast.predictions.days_to_100pct != null && forecast.predictions.days_to_100pct > 0 && (
                                                            <span className="text-sm font-normal ml-2">({forecast.predictions.days_to_100pct} days)</span>
                                                        )}
                                                    </div>
                                                </div>
                                            </div>
                                        )}

                                        {forecast.trend_data && (
                                            <ResponsiveContainer width="100%" height={300}>
                                                <LineChart data={forecast.trend_data}>
                                                    <CartesianGrid strokeDasharray="3 3" stroke="var(--border-color)" />
                                                    <XAxis dataKey="date" stroke="var(--text-secondary)" fontSize={11} />
                                                    <YAxis stroke="var(--text-secondary)" fontSize={12} unit="%" domain={[0, 100]} />
                                                    <Tooltip contentStyle={{ backgroundColor: 'var(--card-bg)', border: '1px solid var(--border-color)' }} />
                                                    <Legend />
                                                    <Line type="monotone" dataKey="actual" stroke="var(--accent-bright)" strokeWidth={2} dot={false} name="Actual" />
                                                    <Line type="monotone" dataKey="trend" stroke="var(--red)" strokeWidth={2} strokeDasharray="5 5" dot={false} name="Trend" />
                                                </LineChart>
                                            </ResponsiveContainer>
                                        )}
                                    </div>
                                )}

                                {forecast?.error && (
                                    <div className="fleet-warnrow text-sm">
                                        {forecast.error}
                                    </div>
                                )}

                                {!forecast && (
                                    <div className="py-8 text-center text-gray-500">
                                        <TrendingUp size={36} className="mx-auto text-gray-300 mb-3" />
                                        <p>Select a server and metric to see capacity predictions.</p>
                                    </div>
                                )}
                            </div>
                        </div>
                    </div>
                )}

                {/* ==================== Fleet Search ==================== */}
                {activeTab === 'search' && (
                    <div className="card">
                        <div className="card-header"><h2>Fleet Search</h2></div>
                        <div className="card-body">
                            <div className="flex gap-3 mb-6">
                                <div className="flex-1">
                                    <Input
                                        type="text"
                                        placeholder="Search for servers, containers, tags..."
                                        value={searchQuery}
                                        onChange={e => setSearchQuery(e.target.value)}
                                        onKeyDown={e => e.key === 'Enter' && handleSearch()}
                                    />
                                </div>
                                <select className="form-select" value={searchType} onChange={e => setSearchType(e.target.value)}>
                                    <option value="any">All Types</option>
                                    <option value="server">Servers</option>
                                    <option value="container">Containers</option>
                                    <option value="tag">Tags</option>
                                </select>
                                <Button onClick={handleSearch} disabled={searchQuery.length < 2}>
                                    <Search size={16} /> Search
                                </Button>
                            </div>

                            {searchResults.length > 0 ? (
                                <div className="overflow-x-auto">
                                    <table className="sk-dtable">
                                        <thead>
                                            <tr>
                                                <th>Server</th>
                                                <th>Type</th>
                                                <th>Match</th>
                                                <th>Detail</th>
                                                <th>Status</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {searchResults.map((r, i) => (
                                                <tr key={i}>
                                                    <td className="font-semibold">{r.server_name}</td>
                                                    <td><Pill kind="gray" dot={false}>{r.match_type}</Pill></td>
                                                    <td>{r.match_name}</td>
                                                    <td className="text-sm text-gray-500">{r.match_detail}</td>
                                                    <td>
                                                        <Pill kind={r.status === 'online' || r.status === 'running' ? 'green' : 'red'}>
                                                            {r.status}
                                                        </Pill>
                                                    </td>
                                                </tr>
                                            ))}
                                        </tbody>
                                    </table>
                                </div>
                            ) : searchQuery.length >= 2 && !loading ? (
                                <div className="py-8 text-center text-gray-500">
                                    <Search size={36} className="mx-auto text-gray-300 mb-3" />
                                    <p>No results found for &quot;{searchQuery}&quot;.</p>
                                </div>
                            ) : (
                                <div className="py-8 text-center text-gray-500">
                                    <Search size={36} className="mx-auto text-gray-300 mb-3" />
                                    <p>Enter a search term to find servers, containers, or tags across your fleet.</p>
                                </div>
                            )}
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
};

export default FleetMonitor;
