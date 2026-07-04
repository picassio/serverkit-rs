import { useState, useEffect } from 'react';
import api from '../../services/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Pill } from '@/components/ds';
import { Zap, Radar, FolderSearch, Download } from 'lucide-react';

const ScannerTab = () => {
    const [scanStatus, setScanStatus] = useState({ status: 'idle' });
    const [scanPath, setScanPath] = useState('/var/www');
    const [scanning, setScanning] = useState(false);
    const [updating, setUpdating] = useState(false);
    const [history, setHistory] = useState([]);
    const [message, setMessage] = useState(null);

    useEffect(() => {
        loadScanStatus();
        loadHistory();
        const interval = setInterval(loadScanStatus, 5000);
        return () => clearInterval(interval);
    }, []);

    async function loadScanStatus() {
        try {
            const data = await api.getScanStatus();
            setScanStatus(data);
        } catch (err) {
            console.error('Failed to load scan status:', err);
        }
    }

    async function loadHistory() {
        try {
            const data = await api.getScanHistory(20);
            setHistory(data.scans || []);
        } catch (err) {
            console.error('Failed to load scan history:', err);
        }
    }

    async function handleStartScan(type) {
        setScanning(true);
        setMessage(null);
        try {
            let result;
            if (type === 'quick') {
                result = await api.runQuickScan();
            } else if (type === 'full') {
                result = await api.runFullScan();
            } else {
                result = await api.scanDirectory(scanPath);
            }
            setMessage({ type: 'success', text: result.message });
            loadScanStatus();
        } catch (err) {
            setMessage({ type: 'error', text: err.message });
        } finally {
            setScanning(false);
        }
    }

    async function handleUpdateDefinitions() {
        setUpdating(true);
        setMessage(null);
        try {
            const result = await api.updateVirusDefinitions();
            setMessage({ type: result.success ? 'success' : 'error', text: result.message || result.error });
        } catch (err) {
            setMessage({ type: 'error', text: err.message });
        } finally {
            setUpdating(false);
        }
    }

    async function handleCancelScan() {
        try {
            await api.cancelScan();
            loadScanStatus();
        } catch (err) {
            setMessage({ type: 'error', text: err.message });
        }
    }

    const isScanning = scanStatus.status === 'running';

    return (
        <div className="scanner-tab">
            {message && (
                <div className={`alert alert-${message.type === 'success' ? 'success' : 'danger'}`}>
                    {message.text}
                </div>
            )}

            <div className="scan-options">
                <div className="scan-card scan-card--quick" onClick={() => !isScanning && !scanning && handleStartScan('quick')}>
                    <div className="scan-card-icon">
                        <Zap size={20} />
                    </div>
                    <h4>Quick Scan</h4>
                    <span className="scan-desc">Scan common web directories</span>
                    <Button
                        variant="default"
                        size="sm"
                        onClick={(e) => { e.stopPropagation(); handleStartScan('quick'); }}
                        disabled={isScanning || scanning}
                    >
                        Start Scan
                    </Button>
                </div>

                <div className="scan-card scan-card--full" onClick={() => !isScanning && !scanning && handleStartScan('full')}>
                    <div className="scan-card-icon">
                        <Radar size={20} />
                    </div>
                    <h4>Full Scan</h4>
                    <span className="scan-desc">Scan entire system (slow)</span>
                    <Button
                        variant="default"
                        size="sm"
                        onClick={(e) => { e.stopPropagation(); handleStartScan('full'); }}
                        disabled={isScanning || scanning}
                    >
                        Start Scan
                    </Button>
                </div>

                <div className="scan-card scan-card--custom">
                    <div className="scan-card-icon">
                        <FolderSearch size={20} />
                    </div>
                    <h4>Custom Path</h4>
                    <span className="scan-desc">Scan a specific directory</span>
                    <div className="scan-custom-input">
                        <Input
                            type="text"
                            value={scanPath}
                            onChange={(e) => setScanPath(e.target.value)}
                            placeholder="/path/to/scan"
                            disabled={isScanning}
                            onClick={(e) => e.stopPropagation()}
                        />
                        <Button
                            variant="default"
                            size="sm"
                            onClick={() => handleStartScan('custom')}
                            disabled={isScanning || scanning || !scanPath}
                        >
                            Scan
                        </Button>
                    </div>
                </div>
            </div>

            <div className="scan-toolbar">
                <Button variant="outline" size="sm" onClick={handleUpdateDefinitions} disabled={updating}>
                    <Download size={14} />
                    {updating ? 'Updating...' : 'Update Definitions'}
                </Button>
            </div>

            {isScanning && (
                <div className="card scan-progress">
                    <div className="card-header">
                        <h3>Scan in Progress</h3>
                        <Button variant="destructive" size="sm" onClick={handleCancelScan}>
                            Cancel
                        </Button>
                    </div>
                    <div className="card-body">
                        <div className="progress-info">
                            <div className="spinner"></div>
                            <div>
                                <p><strong>Scanning:</strong> <span className="sec-mono">{scanStatus.directory}</span></p>
                                <p><strong>Started:</strong> <span className="sec-mono">{new Date(scanStatus.started_at).toLocaleString()}</span></p>
                                {scanStatus.files_scanned > 0 && (
                                    <p><strong>Files scanned:</strong> <span className="sec-mono">{scanStatus.files_scanned}</span></p>
                                )}
                            </div>
                        </div>
                    </div>
                </div>
            )}

            <div className="card sec-flush">
                <div className="card-header">
                    <h3>Scan History</h3>
                    <Button variant="outline" size="sm" onClick={loadHistory}>Refresh</Button>
                </div>
                {history.length === 0 ? (
                    <div className="card-body">
                        <div className="empty-state-sm">
                            <svg viewBox="0 0 24 24" width="40" height="40" stroke="currentColor" fill="none" strokeWidth="1.5">
                                <circle cx="11" cy="11" r="8"/>
                                <line x1="21" y1="21" x2="16.65" y2="16.65"/>
                            </svg>
                            <p>No scans have been run yet. Start a scan above to check for threats.</p>
                        </div>
                    </div>
                ) : (
                    <table className="sk-dtable">
                        <thead>
                            <tr>
                                <th>Date</th>
                                <th>Directory</th>
                                <th>Status</th>
                                <th>Threats</th>
                            </tr>
                        </thead>
                        <tbody>
                            {history.map((scan, index) => (
                                <tr key={index}>
                                    <td className="sk-cell-mono sec-faint">{new Date(scan.started_at).toLocaleString()}</td>
                                    <td className="sk-cell-mono sec-path">{scan.directory}</td>
                                    <td>
                                        <Pill kind={scan.status === 'completed' ? 'green' : scan.status === 'error' ? 'red' : 'amber'}>
                                            {scan.status}
                                        </Pill>
                                    </td>
                                    <td>
                                        {scan.infected_files?.length > 0 ? (
                                            <span className="sec-state sec-state--red">{scan.infected_files.length} found</span>
                                        ) : (
                                            <span className="sec-state sec-state--green">clean</span>
                                        )}
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                )}
            </div>
        </div>
    );
};

export default ScannerTab;
