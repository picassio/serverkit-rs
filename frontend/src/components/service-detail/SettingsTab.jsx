import { useEffect, useState } from 'react';
import { useNavigate, useParams, Link } from 'react-router-dom';
import {
    SlidersHorizontal,
    GitBranch,
    AlertTriangle,
    Globe,
    Lock,
    Shield,
    ShieldCheck,
    Boxes,
    Hammer,
    Archive,
    HardDrive,
    CircleCheck,
    CircleX,
    Sparkles,
} from 'lucide-react';
import api from '../../services/api';
import { useToast } from '../../contexts/ToastContext';
import { DangerZone } from '../DangerZone';
import RepoConnectForm from '../git/RepoConnectForm';
import ProtectionPanel from '../backups/ProtectionPanel';
import ContainerOpsPanel from '../apps/ContainerOpsPanel';
import VolumesPanel from '../apps/VolumesPanel';
import AppWafPanel from '../apps/AppWafPanel';
import BuildTab from '../appdetail/BuildTab';
import DeployTab from '../appdetail/DeployTab';
import Modal from '@/components/Modal';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectTrigger, SelectContent, SelectItem, SelectValue } from '@/components/ui/select';

// Grouped left sub-nav for the service Settings tab — mirrors the WordPress
// detail page's settings layout: an uppercase mono group label per section with
// the existing setting panels on the right. Groups give it structure (and room
// to grow) instead of one long flat stack.
//
// Built per-service so the type-gated sections relocated here from the old top
// tab strip only appear where they apply: Container Ops (Docker only) and WAF
// (Docker + Python, nginx-served). Build + Git & Deploy apply to every type.
function buildSettingsGroups(app) {
    const isDocker = app.isDocker ?? app.app_type === 'docker';
    const isPython = app.isPython ?? ['flask', 'django'].includes(app.app_type);

    return [
        {
            label: 'General',
            items: [
                // Renamed from plain "Environment" to avoid clashing with the
                // top-level "Env Vars" tab that edits runtime environment variables.
                { id: 'environment', label: 'Environment Type', icon: SlidersHorizontal },
                { id: 'domain', label: 'Domain & SSL', icon: Shield },
                ...(isDocker ? [{ id: 'ops', label: 'Container Ops', icon: Boxes }] : []),
            ],
        },
        {
            // "Git & Deploy" merges the old Deploy tab into the repo connection,
            // since both edited repo / branch / auto-deploy. Build sits beside it.
            label: 'Deployment',
            items: [
                { id: 'git', label: 'Git & Deploy', icon: GitBranch },
                { id: 'build', label: 'Build', icon: Hammer },
            ],
        },
        ...((isDocker || isPython) ? [{
            label: 'Security',
            items: [{ id: 'waf', label: 'WAF', icon: ShieldCheck }],
        }] : []),
        {
            label: 'Data',
            items: [
                ...(isDocker ? [{ id: 'storage', label: 'Storage', icon: HardDrive }] : []),
                { id: 'backups', label: 'Backups', icon: Archive },
            ],
        },
        { label: 'Advanced', items: [{ id: 'danger', label: 'Danger Zone', icon: AlertTriangle }] },
    ];
}

const SettingsTab = ({ app, deployConfig, domains, primaryDomain, onUpdate }) => {
    const settingsGroups = buildSettingsGroups(app);
    const settingsItems = settingsGroups.flatMap((g) => g.items);
    const navigate = useNavigate();
    const toast = useToast();
    // Section lives in the URL (/services/:id/settings/:section) so it's
    // shareable and survives a refresh — same as the WordPress detail page.
    const { section: sectionParam } = useParams();
    const section = settingsItems.some((s) => s.id === sectionParam) ? sectionParam : 'environment';
    const setSection = (s) => navigate(`/services/${app.id}/settings/${s}`, { replace: true });
    const [deleting, setDeleting] = useState(false);
    const [environmentType, setEnvironmentType] = useState(app.environment_type || 'standalone');
    const [savingEnvironment, setSavingEnvironment] = useState(false);
    const [unlinking, setUnlinking] = useState(false);

    const envLabels = {
        standalone: 'Standalone',
        production: 'Production',
        development: 'Development',
        staging: 'Staging',
    };

    async function handleEnvironmentChange(newType) {
        if (newType === app.environment_type) return;

        setSavingEnvironment(true);
        try {
            await api.updateAppEnvironment(app.id, newType);
            setEnvironmentType(newType);
            onUpdate();
        } catch (err) {
            toast.error('Failed to update environment type');
            setEnvironmentType(app.environment_type || 'standalone');
        } finally {
            setSavingEnvironment(false);
        }
    }

    async function handleUnlink() {
        if (!confirm(`Unlink ${app.name} from its linked application?`)) return;

        setUnlinking(true);
        try {
            await api.unlinkApp(app.id);
            onUpdate();
        } catch (err) {
            toast.error('Failed to unlink app');
        } finally {
            setUnlinking(false);
        }
    }

    async function handleDelete() {
        if (!confirm(`Delete ${app.name}? This action cannot be undone.`)) return;
        if (!confirm('Are you sure? This will permanently remove the service.')) return;

        setDeleting(true);
        try {
            await api.deleteApp(app.id);
            navigate('/services');
        } catch (err) {
            toast.error('Failed to delete service');
            setDeleting(false);
        }
    }

    // Repo state shaped for the shared RepoConnectForm (the same component the
    // WordPress Git settings use). A connected deploy config == connected repo.
    const gitStatus = {
        connected: Boolean(deployConfig),
        repo_url: deployConfig?.repo_url,
        branch: deployConfig?.branch,
        auto_deploy: deployConfig?.auto_deploy,
        last_deploy_commit: deployConfig?.last_deploy_commit,
        last_deploy_at: deployConfig?.last_deploy_at,
    };

    async function handleConnectRepo(data) {
        const repoUrl = (data.repo_url || '').trim();
        await api.configureDeployment(
            app.id,
            repoUrl,
            data.branch || 'main',
            data.auto_deploy,
            // Preserve any existing deploy scripts (not editable in this form).
            deployConfig?.pre_deploy_script || null,
            deployConfig?.post_deploy_script || null
        );
        if (data.auto_deploy && !deployConfig) {
            try {
                await api.createWebhook({
                    deploy_on_push: true,
                    app_id: app.id,
                    repo_url: repoUrl,
                    branch: data.branch || 'main',
                });
            } catch {
                // Webhook creation is best-effort.
            }
        }
        toast.success('Repository connected');
        onUpdate();
    }

    async function handleDisconnectRepo() {
        await api.removeDeployment(app.id);
        toast.success('Repository disconnected');
        onUpdate();
    }

    return (
        <div className="svc-settings">
            <nav className="svc-settings__nav" aria-label="Service settings sections">
                {settingsGroups.map(g => (
                    <div className="svc-settings__group" key={g.label}>
                        <div className="svc-settings__grouplabel">{g.label}</div>
                        {g.items.map(s => (
                            <button
                                type="button"
                                key={s.id}
                                className={`svc-settings__navitem ${section === s.id ? 'is-active' : ''}`}
                                onClick={() => setSection(s.id)}
                            >
                                <s.icon size={15} />
                                {s.label}
                            </button>
                        ))}
                    </div>
                ))}
            </nav>

            <div className="svc-settings__content">
                {/* Environment Configuration */}
                {section === 'environment' && (
                    <div className="svc-settings__section">
                        <h3 className="svc-settings__section-title">Environment Type</h3>
                        <div className="card settings-section">
                            <div className="settings-row">
                                <div className="settings-label">
                                    <span>Environment Type</span>
                                    <span className="settings-hint">
                                        {app.has_linked_app
                                            ? 'This app is linked. Unlink to change environment type.'
                                            : 'Set how this application is used in your workflow (production, staging, development, or standalone).'}
                                    </span>
                                </div>
                                <div className="settings-control">
                                    {app.has_linked_app ? (
                                        <span className={`env-badge env-${app.environment_type}`}>
                                            {envLabels[app.environment_type] || app.environment_type}
                                        </span>
                                    ) : (
                                        <Select
                                            value={environmentType}
                                            onValueChange={handleEnvironmentChange}
                                            disabled={savingEnvironment}
                                        >
                                            <SelectTrigger className="settings-select">
                                                <SelectValue />
                                            </SelectTrigger>
                                            <SelectContent>
                                                <SelectItem value="standalone">Standalone</SelectItem>
                                                <SelectItem value="development">Development</SelectItem>
                                                <SelectItem value="staging">Staging</SelectItem>
                                                <SelectItem value="production">Production</SelectItem>
                                            </SelectContent>
                                        </Select>
                                    )}
                                    {savingEnvironment && <span className="settings-saving">Saving...</span>}
                                </div>
                            </div>

                            {app.has_linked_app && (
                                <div className="settings-row">
                                    <div className="settings-label">
                                        <span>Linked Application</span>
                                        <span className="settings-hint">
                                            Unlinking will reset both apps to standalone mode.
                                        </span>
                                    </div>
                                    <div className="settings-control">
                                        <Button
                                            variant="outline"
                                            onClick={handleUnlink}
                                            disabled={unlinking}
                                        >
                                            {unlinking ? 'Unlinking...' : 'Unlink'}
                                        </Button>
                                    </div>
                                </div>
                            )}
                        </div>
                    </div>
                )}

                {/* Domain & SSL — same information architecture as WordPress Settings → SSL. */}
                {section === 'domain' && (
                    <div className="svc-settings__section">
                        <h3 className="svc-settings__section-title">Domain &amp; SSL</h3>
                        <DomainSslPanel
                            app={app}
                            domains={domains}
                            primaryDomain={primaryDomain}
                            onUpdate={onUpdate}
                        />
                    </div>
                )}

                {/* Container Ops (Docker only) — image updates, restart policy,
                    resource limits, auto-sleep. Relocated from the old top tab. */}
                {section === 'ops' && (
                    <div className="svc-settings__section">
                        <h3 className="svc-settings__section-title">Container Ops</h3>
                        <ContainerOpsPanel app={app} onChanged={onUpdate} />
                    </div>
                )}

                {/* Storage (Docker only) — first-class managed volumes that
                    survive redeploys, replacing fragile relative bind mounts. */}
                {section === 'storage' && (
                    <div className="svc-settings__section">
                        <h3 className="svc-settings__section-title">Storage</h3>
                        <VolumesPanel app={app} onChanged={onUpdate} />
                    </div>
                )}

                {/* Git & Deploy — the shared RepoConnectForm (provider picker + URL
                    fallback, connected summary with Disconnect) owns the repo
                    connection. Once a repo is linked, the embedded DeployTab adds
                    the deploy pipeline (run actions, deploy scripts, history,
                    config checkpoints) as a separated subsection — no second repo
                    form, and nothing deploy-related shows before connecting. */}
                {section === 'git' && (
                    <div className="svc-settings__section">
                        <h3 className="svc-settings__section-title">Git &amp; Deploy</h3>
                        <RepoConnectForm
                            gitStatus={gitStatus}
                            onConnect={handleConnectRepo}
                            onDisconnect={handleDisconnectRepo}
                            intro={{
                                title: 'Connect a Git repository',
                                subtitle: 'Link a repo so ServerKit can pull your code and redeploy on every push.',
                            }}
                            submitLabel="Connect Repository"
                            idPrefix="svc"
                        />
                        {deployConfig && (
                            <div className="svc-settings__subsection">
                                <h4 className="svc-settings__subsection-title">Deployment</h4>
                                <DeployTab embedded appId={app.id} appPath={app.path} />
                            </div>
                        )}
                    </div>
                )}

                {/* Build — build method, Dockerfile / build pack, cache, timeouts,
                    build logs. Relocated from the old top tab, beside Git & Deploy. */}
                {section === 'build' && (
                    <div className="svc-settings__section">
                        <h3 className="svc-settings__section-title">Build</h3>
                        <BuildTab appId={app.id} appPath={app.path} app={app} />
                    </div>
                )}

                {/* WAF (Docker + Python) — per-app ModSecurity policy + matches. */}
                {section === 'waf' && (
                    <div className="svc-settings__section">
                        <h3 className="svc-settings__section-title">WAF</h3>
                        <AppWafPanel app={app} onChanged={onUpdate} />
                    </div>
                )}

                {/* Backups — the shared Protection panel (scheduled backups,
                    cost, one-click restore), same component the WordPress
                    detail page renders. */}
                {section === 'backups' && (
                    <div className="svc-settings__section">
                        <h3 className="svc-settings__section-title">Backups</h3>
                        <ProtectionPanel
                            targetType="application"
                            targetId={app.id}
                            targetName={app.name}
                        />
                    </div>
                )}

                {/* Danger Zone */}
                {section === 'danger' && (
                    <div className="svc-settings__section">
                        <h3 className="svc-settings__section-title">Danger Zone</h3>
                        <DangerZone
                            description="Once you delete a service, there is no going back. All data will be permanently removed."
                            action={
                                <Button variant="destructive" onClick={handleDelete} disabled={deleting}>
                                    {deleting ? 'Deleting...' : 'Delete Service'}
                                </Button>
                            }
                        />
                    </div>
                )}
            </div>
        </div>
    );
};

// Domain + SSL panel for services. Mirrors the WordPress SiteSSLPanel UX:
// show current primary domain, SSL health, attach a new domain, and request a
// certificate from Let&apos;s Encrypt.
const DomainSslPanel = ({ app, domains, primaryDomain, onUpdate }) => {
    const toast = useToast();
    const [health, setHealth] = useState(null);
    const [checking, setChecking] = useState(false);
    const [issuing, setIssuing] = useState(false);
    const [domainInput, setDomainInput] = useState('');
    const [attaching, setAttaching] = useState(false);
    const [serverkitDomains, setServerkitDomains] = useState([]);
    const [contextLoading, setContextLoading] = useState(true);
    const [email, setEmail] = useState(() => localStorage.getItem('serverkit_ssl_email') || '');

    // "Give it a subdomain" — publish the app at a managed <label>.<base> host.
    const [subdomainModal, setSubdomainModal] = useState(null); // { base_domain, dns_mode }
    const [subdomainLabel, setSubdomainLabel] = useState('');
    const [suggesting, setSuggesting] = useState(false);
    const [publishing, setPublishing] = useState(false);
    // Base domains the app can be published under, and the chosen one, so a
    // multi-base install can pick which domain the subdomain lives on.
    const [subdomainBases, setSubdomainBases] = useState([]);
    const [subdomainBase, setSubdomainBase] = useState('');

    const isPublicDomain = !!primaryDomain
        && !/^(localhost|127\.|10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.)/.test(primaryDomain)
        && primaryDomain.includes('.');

    useEffect(() => {
        if (!primaryDomain) { setChecking(false); setHealth(null); return; }
        let cancelled = false;
        (async () => {
            setChecking(true);
            try {
                const res = await api.getSSLHealth(primaryDomain);
                if (!cancelled) setHealth(res);
            } catch (err) {
                if (!cancelled) setHealth({ valid: false, error: err.message });
            } finally {
                if (!cancelled) setChecking(false);
            }
        })();
        return () => { cancelled = true; };
    }, [primaryDomain]);

    useEffect(() => {
        let cancelled = false;
        (async () => {
            setContextLoading(true);
            try {
                const domainsRes = await api.getDomains().then(d => d.domains || []).catch(() => []);
                if (!cancelled) setServerkitDomains(domainsRes);
            } finally {
                if (!cancelled) setContextLoading(false);
            }
        })();
        return () => { cancelled = true; };
    }, []);

    async function handleAttachDomain(e) {
        e?.preventDefault();
        const name = domainInput.trim();
        if (!name) { toast.error('Enter a domain name'); return; }
        if (!/^[a-z0-9.-]+\.[a-z]{2,}$/i.test(name)) { toast.error('Enter a valid domain name'); return; }

        setAttaching(true);
        try {
            const res = await api.createDomain({
                name,
                application_id: app.id,
                is_primary: domains.length === 0,
                ssl_enabled: false,
            });
            toast.success(res.message || 'Domain attached');
            setDomainInput('');
            onUpdate();
        } catch (err) {
            toast.error(err.message || 'Failed to attach domain');
        } finally {
            setAttaching(false);
        }
    }

    async function handleSuggestSubdomain() {
        setSuggesting(true);
        try {
            const [suggestRes, basesRes] = await Promise.all([
                api.suggestSubdomain(app.id),
                api.getSiteBaseDomains().catch(() => ({ base_domains: [], default: null })),
            ]);
            if (!suggestRes.base_domain) {
                toast.info('Set a managed-sites base domain in Settings → Sites to publish on a subdomain.', 6000);
                return;
            }
            const bases = basesRes.base_domains || [];
            const chosen = basesRes.default || suggestRes.base_domain;
            // Prefill the editable label from the suggestion (<label>.<base>).
            const suggestedLabel = (suggestRes.suggestion || '').replace(`.${suggestRes.base_domain}`, '');
            setSubdomainLabel(suggestedLabel);
            setSubdomainBases(bases);
            setSubdomainBase(chosen);
            setSubdomainModal({ base_domain: chosen, dns_mode: suggestRes.dns_mode });
        } catch (err) {
            toast.error(err.message || 'Failed to suggest a subdomain');
        } finally {
            setSuggesting(false);
        }
    }

    async function handleGiveSubdomain() {
        setPublishing(true);
        try {
            const res = await api.giveSubdomain(app.id, subdomainLabel.trim(), subdomainBase || undefined);
            if (res.success) {
                if (res.warning) toast.warning(res.warning);
                toast.success(res.url ? `Published at ${res.url}` : 'Subdomain published');
                setSubdomainModal(null);
                setSubdomainLabel('');
                onUpdate();
            } else {
                toast.error(res.error || 'Failed to publish subdomain');
            }
        } catch (err) {
            toast.error(err.message || 'Failed to publish subdomain');
        } finally {
            setPublishing(false);
        }
    }

    async function handleEnableSSL() {
        if (!primaryDomain) { toast.error('No primary domain configured'); return; }
        if (!email.trim() || !email.includes('@')) { toast.error('Enter a valid email for certificate expiry notices'); return; }
        localStorage.setItem('serverkit_ssl_email', email.trim());
        setIssuing(true);
        toast.info(`Requesting certificate for ${primaryDomain}...`, { duration: 4000 });
        try {
            const res = await api.obtainCertificate({
                domains: [primaryDomain],
                email,
                use_nginx: true,
            });
            if (res.success) {
                toast.success(res.message || 'Certificate issued');
                // Mark the domain as SSL-enabled in ServerKit.
                const primary = domains.find(d => d.is_primary) || domains[0];
                if (primary?.id) {
                    await api.updateDomain(primary.id, { ssl_enabled: true, ssl_auto_renew: true }).catch(() => {});
                }
                const updated = await api.getSSLHealth(primaryDomain);
                setHealth(updated);
                onUpdate();
            } else {
                toast.error(res.error || 'Certificate request failed');
            }
        } catch (err) {
            toast.error(err.message || 'Certificate request failed');
        } finally {
            setIssuing(false);
        }
    }

    const issued = health?.valid;
    const attachValid = /^[a-z0-9.-]+\.[a-z]{2,}$/i.test(domainInput.trim());

    const CheckItem = ({ ok, label }) => (
        <div className="ssl-check-item">
            {ok ? <CircleCheck size={14} className="ssl-check-icon ssl-check-icon--ok" /> : <CircleX size={14} className="ssl-check-icon ssl-check-icon--missing" />}
            <span className={ok ? 'ssl-check-label ssl-check-label--ok' : 'ssl-check-label'}>{label}</span>
        </div>
    );

    return (
        <div className="card settings-section svc-domain-panel">
            <div className="app-info-grid">
                <div className="app-info-item">
                    <span className="app-info-label">Primary Domain</span>
                    <span className="app-info-value mono">{primaryDomain || 'None configured'}</span>
                </div>
                <div className="app-info-item">
                    <span className="app-info-label">SSL Status</span>
                    <span className="app-info-value">
                        {!primaryDomain ? '—' : checking ? 'Checking…' : issued ? 'Active' : 'Not Secured'}
                    </span>
                </div>
                {issued && health.expires_at && (
                    <div className="app-info-item">
                        <span className="app-info-label">Expires</span>
                        <span className="app-info-value">
                            {new Date(health.expires_at).toLocaleDateString()}
                            {typeof health.days_remaining === 'number' ? ` (${health.days_remaining}d)` : ''}
                        </span>
                    </div>
                )}
                {issued && health.issuer && (
                    <div className="app-info-item">
                        <span className="app-info-label">Issuer</span>
                        <span className="app-info-value">{health.issuer}</span>
                    </div>
                )}
            </div>

            {/* One-click publish at a managed subdomain (<slug>.<base>). Works
                whether or not a domain is already attached. */}
            <div className="svc-give-subdomain">
                <Button variant="outline" onClick={handleSuggestSubdomain} disabled={suggesting}>
                    <Sparkles size={14} />
                    {suggesting ? 'Checking…' : 'Give it a subdomain'}
                </Button>
                <span className="form-hint">Publish this service at a ServerKit-managed subdomain.</span>
            </div>

            {!primaryDomain ? (
                <div className="ssl-guide">
                    <p className="hint">No domain is attached to this service yet. Add one to expose it on a public URL and enable HTTPS.</p>
                    <form className="ssl-inline-attach" onSubmit={handleAttachDomain}>
                        {contextLoading ? (
                            <p className="hint">Loading available domains…</p>
                        ) : (
                            <div className="ssl-context">
                                <div className="ssl-context-links">
                                    <Link to="/domains">Manage domains</Link>
                                </div>
                            </div>
                        )}
                        <div className="form-group">
                            <Label>Domain</Label>
                            <Input
                                type="text"
                                value={domainInput}
                                onChange={(e) => setDomainInput(e.target.value)}
                                placeholder="example.com"
                                disabled={attaching}
                                list="svc-existing-domains"
                            />
                            <datalist id="svc-existing-domains">
                                {serverkitDomains
                                    .filter(d => !domains.some(ad => ad.name === d.name))
                                    .map(d => (
                                        <option key={d.id} value={d.name}>
                                            {d.ssl_enabled ? 'SSL enabled' : 'No SSL'}
                                        </option>
                                    ))}
                            </datalist>
                            <span className="form-hint">Pick an existing ServerKit domain or type one you control, without http://</span>
                        </div>
                        <div className="app-detail-actions">
                            <Button type="submit" disabled={!attachValid || attaching}>
                                <Globe size={14} />
                                {attaching ? 'Attaching…' : 'Attach Domain'}
                            </Button>
                        </div>
                    </form>
                </div>
            ) : !isPublicDomain ? (
                <div className="ssl-guide">
                    <p className="hint">SSL requires a public domain pointed at this server. This site is on <code>{primaryDomain}</code>, so a certificate cannot be issued here.</p>
                    <div className="ssl-checklist">
                        <CheckItem ok={false} label="Public domain mapped to this service" />
                    </div>
                </div>
            ) : (
                <div className="ssl-guide">
                    {issued ? (
                        <p className="hint">This service is secured with a valid SSL certificate. You can re-issue it if needed.</p>
                    ) : (
                        <>
                            <p className="hint">Enter the email Let&apos;s Encrypt should use for expiry notices, then request a free certificate.</p>
                            <div className="ssl-checklist">
                                <CheckItem ok label={`Domain ${primaryDomain} configured`} />
                            </div>
                            <div className="form-group">
                                <Label>Admin Email</Label>
                                <Input
                                    type="email"
                                    value={email}
                                    onChange={(e) => setEmail(e.target.value)}
                                    placeholder="admin@example.com"
                                    disabled={issuing}
                                />
                                <span className="form-hint">Used for certificate expiry reminders from Let&apos;s Encrypt.</span>
                            </div>
                        </>
                    )}
                    <div className="app-detail-actions">
                        <Button onClick={handleEnableSSL} disabled={issuing || (!issued && (!email.trim() || !email.includes('@')))}>
                            {issued ? <Shield size={14} /> : <Lock size={14} />}
                            {issuing ? 'Requesting...' : issued ? 'Re-issue Certificate' : 'Enable SSL'}
                        </Button>
                    </div>
                </div>
            )}

            <Modal
                open={!!subdomainModal}
                onClose={() => setSubdomainModal(null)}
                title="Give it a subdomain"
            >
                {subdomainModal && (
                    <>
                        <div className="modal-body">
                            <p className="hint">
                                Publish <strong>{app.name}</strong> at a managed subdomain of{' '}
                                <code>{subdomainBase || subdomainModal.base_domain}</code>.
                            </p>
                            {subdomainBases.length > 1 && (
                                <div className="form-group">
                                    <Label>Base domain</Label>
                                    <select
                                        className="settings-select"
                                        value={subdomainBase}
                                        onChange={(e) => setSubdomainBase(e.target.value)}
                                        disabled={publishing}
                                    >
                                        {subdomainBases.map((b) => (
                                            <option key={b.domain} value={b.domain}>
                                                {b.domain}{b.is_default ? ' (default)' : ''}{b.https_enabled ? ' — HTTPS' : ''}
                                            </option>
                                        ))}
                                    </select>
                                    <span className="form-hint">Which registered base domain to publish this service under.</span>
                                </div>
                            )}
                            <div className="form-group">
                                <Label>Subdomain</Label>
                                <div className="svc-subdomain-input">
                                    <Input
                                        type="text"
                                        value={subdomainLabel}
                                        onChange={(e) => setSubdomainLabel(e.target.value)}
                                        placeholder="my-app"
                                        disabled={publishing}
                                    />
                                    <span className="svc-subdomain-input__suffix">.{subdomainBase || subdomainModal.base_domain}</span>
                                </div>
                                <span className="form-hint">
                                    {(subdomainBases.find((b) => b.domain === subdomainBase)?.dns_mode || subdomainModal.dns_mode) === 'wildcard'
                                        ? 'Wildcard DNS is configured — this resolves instantly, no record needed.'
                                        : 'Per-site mode — a DNS record will be created for this host.'}
                                </span>
                            </div>
                        </div>
                        <div className="modal-footer">
                            <Button variant="outline" onClick={() => setSubdomainModal(null)} disabled={publishing}>Cancel</Button>
                            <Button onClick={handleGiveSubdomain} disabled={publishing || !subdomainLabel.trim()}>
                                {publishing ? 'Publishing…' : 'Publish'}
                            </Button>
                        </div>
                    </>
                )}
            </Modal>
        </div>
    );
};

export default SettingsTab;
