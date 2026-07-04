import { useState, useEffect } from 'react';
import api from '../../services/api';
import { useToast } from '../../contexts/ToastContext';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import EmptyState from '../EmptyState';

const PackagesTab = ({ appId }) => {
    const toast = useToast();
    const [packages, setPackages] = useState([]);
    const [loading, setLoading] = useState(true);
    const [installing, setInstalling] = useState(false);
    const [newPackage, setNewPackage] = useState('');

    useEffect(() => {
        loadPackages();
    }, [appId]);

    async function loadPackages() {
        try {
            const data = await api.getPythonPackages(appId);
            setPackages(data.packages || []);
        } catch (err) {
            console.error('Failed to load packages:', err);
        } finally {
            setLoading(false);
        }
    }

    async function handleInstall(e) {
        e.preventDefault();
        if (!newPackage.trim()) return;

        setInstalling(true);
        try {
            await api.installPythonPackages(appId, [newPackage.trim()]);
            setNewPackage('');
            loadPackages();
        } catch (err) {
            console.error('Failed to install package:', err);
        } finally {
            setInstalling(false);
        }
    }

    async function handleFreeze() {
        try {
            await api.freezePythonRequirements(appId);
            toast.success('requirements.txt updated');
        } catch (err) {
            toast.error('Failed to freeze requirements');
        }
    }

    if (loading) {
        return <EmptyState loading title="Loading packages..." />;
    }

    return (
        <div>
            <div className="section-header">
                <h3 className="svc-eyebrow">
                    Installed Packages <span className="svc-eyebrow__count">&middot; {packages.length}</span>
                </h3>
                <Button variant="outline" size="sm" onClick={handleFreeze}>
                    Freeze to requirements.txt
                </Button>
            </div>

            <form className="install-form" onSubmit={handleInstall}>
                <Input
                    type="text"
                    value={newPackage}
                    onChange={(e) => setNewPackage(e.target.value)}
                    placeholder="Package name (e.g., requests, flask==2.0.0)"
                />
                <Button type="submit" disabled={installing}>
                    {installing ? 'Installing...' : 'Install'}
                </Button>
            </form>

            <div className="svc-card">
                <table className="sk-dtable">
                    <thead>
                        <tr>
                            <th>Package</th>
                            <th>Version</th>
                        </tr>
                    </thead>
                    <tbody>
                        {packages.map(pkg => (
                            <tr key={pkg.name}>
                                <td className="sk-cell-mono svc-pkg-name">{pkg.name}</td>
                                <td className="sk-cell-mono">{pkg.version}</td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            </div>
        </div>
    );
};

export default PackagesTab;
