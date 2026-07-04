import { useState, useEffect } from 'react';
import { useAuth } from '../../contexts/AuthContext';
import api from '../../services/api';
import { InfoList, InfoItem } from '../InfoList';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { Select, SelectTrigger, SelectContent, SelectItem, SelectValue } from '@/components/ui/select';
import { formatBytes } from '@/utils/formatBytes';
import EmptyState from '../EmptyState';

function formatUptime(seconds) {
    if (!seconds) return '-';
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);

    const parts = [];
    if (days > 0) parts.push(`${days}d`);
    if (hours > 0) parts.push(`${hours}h`);
    if (minutes > 0) parts.push(`${minutes}m`);

    return parts.join(' ') || '< 1m';
}

const SystemTab = () => {
    const { isAdmin } = useAuth();
    const [metrics, setMetrics] = useState(null);
    const [loading, setLoading] = useState(true);
    const [timezones, setTimezones] = useState([]);
    const [selectedTimezone, setSelectedTimezone] = useState('');
    const [savingTimezone, setSavingTimezone] = useState(false);
    const [timezoneMessage, setTimezoneMessage] = useState(null);

    const [domainLoading, setDomainLoading] = useState(false);
    const [detectedDomain, setDetectedDomain] = useState(null);
    const [canonicalDomain, setCanonicalDomain] = useState('');
    const [canonicalHttps, setCanonicalHttps] = useState(false);
    const [encryptionConfigured, setEncryptionConfigured] = useState(true);
    const [savingDomain, setSavingDomain] = useState(false);
    const [domainMessage, setDomainMessage] = useState(null);

    useEffect(() => {
        if (isAdmin) {
            loadMetrics();
            loadTimezones();
            loadDomainInfo();
        }
    }, [isAdmin]);

    async function loadMetrics() {
        try {
            const data = await api.getSystemMetrics();
            setMetrics(data);
            if (data?.time?.timezone_id) {
                setSelectedTimezone(data.time.timezone_id);
            }
        } catch (err) {
            console.error('Failed to load metrics:', err);
        } finally {
            setLoading(false);
        }
    }

    async function loadTimezones() {
        try {
            const data = await api.getTimezones();
            setTimezones(data.timezones || []);
        } catch (err) {
            console.error('Failed to load timezones:', err);
        }
    }

    async function loadDomainInfo() {
        setDomainLoading(true);
        try {
            const [detection, health] = await Promise.all([
                api.getDomainDetection(),
                api.healthCheck()
            ]);
            setDetectedDomain(detection);
            setCanonicalDomain(detection.current_canonical_domain || '');
            setCanonicalHttps(detection.current_canonical_https_enabled || false);
            setEncryptionConfigured(health.encryption_configured !== false);
        } catch (err) {
            console.error('Failed to load domain info:', err);
        } finally {
            setDomainLoading(false);
        }
    }

    async function handleSaveCanonicalDomain() {
        setSavingDomain(true);
        setDomainMessage(null);
        try {
            const result = await api.setCanonicalDomain(canonicalDomain, canonicalHttps);
            setDomainMessage({
                type: 'success',
                text: `${result.message}. Restart the ServerKit backend service for CORS changes to take full effect.`
            });
            loadDomainInfo();
        } catch (err) {
            setDomainMessage({ type: 'error', text: err.message || 'Failed to save canonical domain' });
        } finally {
            setSavingDomain(false);
            setTimeout(() => setDomainMessage(null), 8000);
        }
    }

    async function handleUseDetectedDomain() {
        if (!detectedDomain?.detected_domain) return;
        setCanonicalDomain(detectedDomain.detected_domain);
        setCanonicalHttps(detectedDomain.is_https);
    }

    async function handleTimezoneChange() {
        if (!selectedTimezone) return;

        setSavingTimezone(true);
        setTimezoneMessage(null);

        try {
            const result = await api.setTimezone(selectedTimezone);
            setTimezoneMessage({ type: 'success', text: result.message || 'Timezone updated' });
            // Refresh metrics to show new time
            loadMetrics();
        } catch (err) {
            setTimezoneMessage({ type: 'error', text: err.message || 'Failed to set timezone' });
        } finally {
            setSavingTimezone(false);
            setTimeout(() => setTimezoneMessage(null), 5000);
        }
    }

    if (!isAdmin) {
        return (
            <div className="settings-section">
                <div className="section-header">
                    <h2>System Information</h2>
                    <p>View system details and server information</p>
                </div>
                <div className="alert alert-warning">
                    Admin access required to view system information.
                </div>
            </div>
        );
    }

    if (loading) {
        return <EmptyState loading title="Loading system information..." />;
    }

    return (
        <div className="settings-section">
            <div className="section-header">
                <h2>System Information</h2>
                <p>View system details and server information</p>
            </div>

            <div className="system-info-grid">
                <div className="settings-card">
                    <h3>CPU</h3>
                    <InfoList>
                        <InfoItem label="Usage" value={`${metrics?.cpu?.percent?.toFixed(1) || 0}%`} />
                        <InfoItem label="Cores" value={metrics?.cpu?.count || '-'} />
                        <InfoItem
                            label="Load Average"
                            value={metrics?.cpu?.load_avg ? metrics.cpu.load_avg.map(l => l.toFixed(2)).join(', ') : '-'}
                        />
                    </InfoList>
                </div>

                <div className="settings-card">
                    <h3>Memory</h3>
                    <InfoList>
                        <InfoItem label="Usage" value={`${metrics?.memory?.percent?.toFixed(1) || 0}%`} />
                        <InfoItem label="Used" value={formatBytes(metrics?.memory?.used)} />
                        <InfoItem label="Total" value={formatBytes(metrics?.memory?.total)} />
                    </InfoList>
                </div>

                <div className="settings-card">
                    <h3>Disk</h3>
                    <InfoList>
                        <InfoItem label="Usage" value={`${metrics?.disk?.percent?.toFixed(1) || 0}%`} />
                        <InfoItem label="Used" value={formatBytes(metrics?.disk?.used)} />
                        <InfoItem label="Total" value={formatBytes(metrics?.disk?.total)} />
                    </InfoList>
                </div>

                <div className="settings-card">
                    <h3>Network</h3>
                    <InfoList>
                        <InfoItem label="Bytes Sent" value={formatBytes(metrics?.network?.bytes_sent)} />
                        <InfoItem label="Bytes Received" value={formatBytes(metrics?.network?.bytes_recv)} />
                    </InfoList>
                </div>
            </div>

            {metrics?.system && (
                <div className="settings-card">
                    <h3>System Details</h3>
                    <InfoList>
                        <InfoItem label="Hostname" value={metrics.system.hostname || '-'} />
                        <InfoItem label="Platform" value={metrics.system.platform || '-'} />
                        <InfoItem label="OS Version" value={metrics.system.version || '-'} />
                        <InfoItem label="Uptime" value={formatUptime(metrics.system.uptime)} />
                    </InfoList>
                </div>
            )}

            {/* Server Time & Timezone */}
            <div className="settings-card">
                <h3>Server Time & Timezone</h3>
                {metrics?.time && (
                    <InfoList style={{ marginBottom: '1rem' }}>
                        <InfoItem label="Current Time" value={metrics.time.current_time_formatted} />
                        <InfoItem label="UTC Offset" value={metrics.time.utc_offset} />
                        <InfoItem label="Current Timezone" value={metrics.time.timezone_id || metrics.time.timezone_name} />
                    </InfoList>
                )}
                <div className="form-group">
                    <label>Change Timezone</label>
                    <div className="timezone-selector">
                        <Select
                            value={selectedTimezone || '__none__'}
                            onValueChange={(val) => setSelectedTimezone(val === '__none__' ? '' : val)}
                        >
                            <SelectTrigger>
                                <SelectValue placeholder="Select timezone..." />
                            </SelectTrigger>
                            <SelectContent>
                                <SelectItem value="__none__">Select timezone...</SelectItem>
                                {timezones.map((tz) => (
                                    <SelectItem key={tz} value={tz}>{tz}</SelectItem>
                                ))}
                            </SelectContent>
                        </Select>
                        <Button
                            variant="default"
                            onClick={handleTimezoneChange}
                            disabled={savingTimezone || !selectedTimezone || selectedTimezone === metrics?.time?.timezone_id}
                        >
                            {savingTimezone ? 'Saving...' : 'Apply'}
                        </Button>
                    </div>
                    {timezoneMessage && (
                        <div className={`timezone-message ${timezoneMessage.type}`}>
                            {timezoneMessage.text}
                        </div>
                    )}
                    <span className="form-help">
                        Changing timezone requires server restart to take full effect
                    </span>
                </div>
            </div>

            {/* Panel Domain */}
            <div className="settings-card">
                <h3>Panel Domain</h3>
                {!encryptionConfigured && (
                    <div className="alert alert-warning" style={{ marginBottom: '1rem' }}>
                        <strong>Encryption key not configured.</strong> Agent pairing and secret encryption
                        will fail until SERVERKIT_ENCRYPTION_KEY is set in your .env file.
                    </div>
                )}
                {domainLoading ? (
                    <EmptyState loading title="Loading domain settings..." />
                ) : (
                    <>
                        {detectedDomain?.detected_domain && (
                            <div className="form-group">
                                <label>Detected Domain</label>
                                <div className="form-row" style={{ display: 'flex', gap: '0.75rem', alignItems: 'center' }}>
                                    <code style={{ flex: 1 }}>
                                        {detectedDomain.is_https ? 'https' : 'http'}://{detectedDomain.detected_domain}
                                    </code>
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        onClick={handleUseDetectedDomain}
                                        disabled={savingDomain}
                                    >
                                        Use this domain
                                    </Button>
                                </div>
                                <span className="form-help">
                                    Detected from the Host header of your current request
                                </span>
                            </div>
                        )}
                        {detectedDomain?.current_canonical_domain && (
                            <div className="form-group">
                                <label>Current Canonical Domain</label>
                                <div>
                                    <code>{detectedDomain.current_canonical_origin || '-'}</code>
                                </div>
                            </div>
                        )}
                        <div className="form-group">
                            <label htmlFor="canonical-domain">Canonical Domain</label>
                            <Input
                                id="canonical-domain"
                                value={canonicalDomain}
                                onChange={(e) => setCanonicalDomain(e.target.value)}
                                placeholder="e.g. serverkit.example.com"
                                disabled={savingDomain}
                            />
                            <span className="form-help">
                                The domain you point at this ServerKit panel. Used for CORS and agent install commands.
                            </span>
                        </div>
                        <div className="form-group" style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
                            <Switch
                                id="canonical-https"
                                checked={canonicalHttps}
                                onCheckedChange={setCanonicalHttps}
                                disabled={savingDomain}
                            />
                            <label htmlFor="canonical-https" style={{ margin: 0 }}>
                                HTTPS enabled for canonical domain
                            </label>
                        </div>
                        <Button
                            variant="default"
                            onClick={handleSaveCanonicalDomain}
                            disabled={savingDomain || !canonicalDomain}
                        >
                            {savingDomain ? 'Saving...' : 'Save Canonical Domain'}
                        </Button>
                        {domainMessage && (
                            <div className={`timezone-message ${domainMessage.type}`} style={{ marginTop: '0.75rem' }}>
                                {domainMessage.text}
                            </div>
                        )}
                    </>
                )}
            </div>
        </div>
    );
};

export default SystemTab;
