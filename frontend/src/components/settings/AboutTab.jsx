import { useState, useEffect } from 'react';
import api from '../../services/api';
import {
    Github, FileText, HelpCircle, MessageSquare, Bug, Check, Download, CheckCircle,
    RefreshCw, ExternalLink, Star, X
} from 'lucide-react';
import ServerKitLogo from '../ServerKitLogo';
import { Button } from '@/components/ui/button';

const STAR_PROMPT_KEY = 'serverkit-star-prompt-dismissed';

const AboutTab = () => {
    const [version, setVersion] = useState('...');
    const [updateInfo, setUpdateInfo] = useState(null);
    const [checkingUpdate, setCheckingUpdate] = useState(false);
    const [showStarPrompt, setShowStarPrompt] = useState(() => {
        return localStorage.getItem(STAR_PROMPT_KEY) !== 'true';
    });

    useEffect(() => {
        const fetchVersion = async () => {
            try {
                const data = await api.getVersion();
                setVersion(data.version || '1.0.0');
            } catch (error) {
                setVersion('1.0.0');
            }
        };
        fetchVersion();
    }, []);

    const dismissStarPrompt = () => {
        setShowStarPrompt(false);
        localStorage.setItem(STAR_PROMPT_KEY, 'true');
    };

    const checkForUpdate = async () => {
        setCheckingUpdate(true);
        try {
            const data = await api.checkUpdate();
            setUpdateInfo(data);
        } catch (error) {
            setUpdateInfo({ error: 'Failed to check for updates' });
        }
        setCheckingUpdate(false);
    };

    return (
        <div className="settings-section">
            <div className="section-header">
                <h2>About ServerKit</h2>
                <p>Server management made simple</p>
            </div>

            <div className="about-card">
                <div className="about-logo">
                    <ServerKitLogo width={64} height={64} />
                </div>
                <h3>ServerKit</h3>
                <p className="version">Version {version}</p>
                <p className="description">
                    A modern, lightweight server management panel for managing web applications,
                    databases, domains, and more. Built with Flask and React.
                </p>

                <div className="update-check">
                    {!updateInfo ? (
                        <Button
                            variant="outline"
                            size="sm"
                            onClick={checkForUpdate}
                            disabled={checkingUpdate}
                        >
                            {checkingUpdate ? (
                                <><RefreshCw size={14} className="spinning" /> Checking...</>
                            ) : (
                                <><Download size={14} /> Check for Updates</>
                            )}
                        </Button>
                    ) : updateInfo.error ? (
                        <div className="update-status error">
                            <span>{updateInfo.error}</span>
                            <button type="button" className="btn-link" onClick={checkForUpdate}>Retry</button>
                        </div>
                    ) : updateInfo.update_available ? (
                        <div className="update-status available">
                            <Download size={16} />
                            <span>Update available: <strong>v{updateInfo.latest_version}</strong></span>
                            <a
                                href={updateInfo.release_url}
                                target="_blank"
                                rel="noopener noreferrer"
                                className="btn btn-accent btn-sm"
                            >
                                View Release <ExternalLink size={12} />
                            </a>
                        </div>
                    ) : (
                        <div className="update-status current">
                            <CheckCircle size={16} />
                            <span>You&apos;re up to date!</span>
                        </div>
                    )}
                </div>
            </div>

            {showStarPrompt && (
                <div className="star-prompt-card">
                    <button type="button" className="dismiss-btn" onClick={dismissStarPrompt} title="Dismiss">
                        <X size={16} />
                    </button>
                    <div className="star-icon">
                        <Star size={24} />
                    </div>
                    <div className="star-content">
                        <h4>Enjoying ServerKit?</h4>
                        <p>If you find ServerKit useful, consider starring the repository on GitHub. It helps others discover the project!</p>
                        <a
                            href="https://github.com/jhd3197/ServerKit"
                            target="_blank"
                            rel="noopener noreferrer"
                            className="btn btn-accent"
                        >
                            <Star size={16} />
                            Star on GitHub
                        </a>
                    </div>
                </div>
            )}

            <div className="settings-card">
                <h3>Features</h3>
                <ul className="feature-list">
                    <li>
                        <Check size={16} />
                        Application Management (PHP, Python, Node.js, Docker)
                    </li>
                    <li>
                        <Check size={16} />
                        Domain & SSL Certificate Management
                    </li>
                    <li>
                        <Check size={16} />
                        Database Management (MySQL, PostgreSQL)
                    </li>
                    <li>
                        <Check size={16} />
                        Docker Container Management
                    </li>
                    <li>
                        <Check size={16} />
                        System Monitoring & Alerts
                    </li>
                    <li>
                        <Check size={16} />
                        Automated Backups
                    </li>
                    <li>
                        <Check size={16} />
                        Git Deployment with Webhooks
                    </li>
                </ul>
            </div>

            <div className="settings-card">
                <h3>Links</h3>
                <div className="link-list">
                    <a href="https://github.com/jhd3197/ServerKit" target="_blank" rel="noopener noreferrer" className="link-item">
                        <Github size={18} />
                        GitHub Repository
                    </a>
                    <a href="https://github.com/jhd3197/ServerKit#readme" target="_blank" rel="noopener noreferrer" className="link-item">
                        <FileText size={18} />
                        Documentation
                    </a>
                    <a href="https://github.com/jhd3197/ServerKit/issues" target="_blank" rel="noopener noreferrer" className="link-item">
                        <HelpCircle size={18} />
                        Support & Issues
                    </a>
                    <a href="https://github.com/jhd3197/ServerKit/discussions" target="_blank" rel="noopener noreferrer" className="link-item">
                        <MessageSquare size={18} />
                        Discussions
                    </a>
                    <a href="https://github.com/jhd3197/ServerKit/issues/new" target="_blank" rel="noopener noreferrer" className="link-item">
                        <Bug size={18} />
                        Report a Bug
                    </a>
                </div>
            </div>

            <div className="settings-card">
                <h3>License</h3>
                <p className="license-text">
                    ServerKit is open source software licensed under the MIT License.
                </p>
            </div>
        </div>
    );
};

export default AboutTab;
