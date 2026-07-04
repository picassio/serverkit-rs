import { useCallback, useEffect, useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import {
    ArrowRight,
    CheckCircle2,
    ChevronDown,
    FileArchive,
    FolderOpen,
    GitBranch,
    Link2,
    Lock,
    Network,
    Package,
    RefreshCw,
    Rocket,
    Search,
    Server,
    Settings2,
    ShieldCheck,
    Zap,
} from 'lucide-react';
import { SiGithub } from 'react-icons/si';
import api from '../services/api';
import { useToast } from '../contexts/ToastContext';
import { useTopbarActions } from '@/hooks/useTopbarActions';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import BuildpackPreview from '@/components/buildpack/BuildpackPreview';

const APP_TYPE_OPTIONS = [
    { value: 'auto', label: 'Auto-detect' },
    { value: 'docker', label: 'Docker / Compose' },
    { value: 'flask', label: 'Python' },
    { value: 'django', label: 'Django' },
    { value: 'php', label: 'PHP' },
    { value: 'static', label: 'Static site' },
];

const BUILD_METHOD_OPTIONS = [
    { value: 'auto', label: 'Auto build' },
    { value: 'nixpacks', label: 'Nixpacks' },
    { value: 'dockerfile', label: 'Dockerfile' },
    { value: 'custom', label: 'Custom command' },
];

const APP_TYPE_LABELS = Object.fromEntries(APP_TYPE_OPTIONS.map(option => [option.value, option.label]));
const BUILD_METHOD_LABELS = Object.fromEntries(BUILD_METHOD_OPTIONS.map(option => [option.value, option.label]));

const SERVICE_TEMPLATES = [
    {
        id: 'agentsite',
        name: 'AgentSite',
        serviceName: 'agentsite',
        description: 'AI-powered website builder with multi-agent orchestration.',
        repoUrl: 'https://github.com/jhd3197/AgentSite.git',
        branch: 'main',
        appType: 'docker',
        buildMethod: 'dockerfile',
        port: 6391,
        badges: ['Render', 'Railway', 'Compose', 'Dockerfile'],
        manifest: {
            strategy: 'docker_compose',
            recommended: {
                app_type: 'docker',
                build_method: 'dockerfile',
                port: 6391,
                dockerfile_path: 'Dockerfile',
                healthcheck_path: '/api/health',
            },
            manifests: [
                { type: 'docker_compose', file: 'docker-compose.yml', label: 'Docker Compose', summary: 'agentsite service on port 6391' },
                { type: 'render', file: 'render.yaml', label: 'Render blueprint', summary: 'agentsite web service using docker' },
                { type: 'railway', file: 'railway.json', label: 'Railway config', summary: 'Dockerfile build with health check' },
                { type: 'app_json', file: 'app.json', label: 'App manifest', summary: 'AI-powered website builder using multi-agent orchestration' },
            ],
            env: [
                { key: 'OPENAI_API_KEY', required: true, secret: true, source: 'render.yaml' },
                { key: 'CLAUDE_API_KEY', required: true, secret: true, source: 'render.yaml' },
                { key: 'GOOGLE_API_KEY', required: true, secret: true, source: 'render.yaml' },
                { key: 'GROQ_API_KEY', required: true, secret: true, source: 'render.yaml' },
                { key: 'GROK_API_KEY', required: true, secret: true, source: 'render.yaml' },
                { key: 'OPENROUTER_API_KEY', required: true, secret: true, source: 'render.yaml' },
            ],
            ports: [6391],
        },
    },
];

function slugify(value) {
    return value.toLowerCase().replace(/[^a-z0-9-]+/g, '-').replace(/^-+|-+$/g, '');
}

function repoNameFromUrl(value) {
    if (!value) return '';
    const cleaned = value.trim().replace(/\.git$/, '');
    const parts = cleaned.split(/[/:]/).filter(Boolean);
    return slugify(parts[parts.length - 1] || '');
}

function normalizeManualRepo(value) {
    const trimmed = value.trim();
    if (!trimmed) return '';
    if (/^[\w.-]+\/[\w.-]+$/.test(trimmed)) return `https://github.com/${trimmed}.git`;
    if (/^github\.com\//i.test(trimmed)) return `https://${trimmed.replace(/\.git$/, '')}.git`;
    return trimmed;
}

function formatAppType(value) {
    return APP_TYPE_LABELS[value] || value || 'Auto-detect';
}

function formatBuildMethod(value) {
    return BUILD_METHOD_LABELS[value] || value || 'Auto build';
}

const NewService = () => {
    const navigate = useNavigate();
    const toast = useToast();
    const [sourceMode, setSourceMode] = useState('github');
    const [githubStatus, setGithubStatus] = useState(null);
    const [repos, setRepos] = useState([]);
    const [reposLoading, setReposLoading] = useState(false);
    const [repoSearch, setRepoSearch] = useState('');
    const [selectedRepo, setSelectedRepo] = useState(null);
    const [selectedTemplate, setSelectedTemplate] = useState(null);
    const [branches, setBranches] = useState([]);
    const [branchesLoading, setBranchesLoading] = useState(false);
    const [repoManifest, setRepoManifest] = useState(null);
    const [repoManifestLoading, setRepoManifestLoading] = useState(false);
    const [manualRepoUrl, setManualRepoUrl] = useState('');
    const [name, setName] = useState('');
    const [nameTouched, setNameTouched] = useState(false);
    const [branch, setBranch] = useState('main');
    const [appType, setAppType] = useState('auto');
    const [buildMethod, setBuildMethod] = useState('auto');
    const [port, setPort] = useState('');
    // Ingress plane: 'nginx' (host Nginx, the default) or 'proxy_stack' (a
    // Dockerized Traefik/Caddy stack). Only honored by the backend for
    // container-eligible app types; everything else is forced to host Nginx.
    const [ingressPlane, setIngressPlane] = useState('nginx');
    const [autoDeploy, setAutoDeploy] = useState(true);
    const [advancedOpen, setAdvancedOpen] = useState(false);
    const [submitting, setSubmitting] = useState(false);

    // Build-pack detection (zero-Dockerfile). Runs when a repo is selected and
    // the build method routes through the build pack (auto / nixpacks).
    const [buildpack, setBuildpack] = useState(null);
    const [buildpackLoading, setBuildpackLoading] = useState(false);
    const [buildpackOverrides, setBuildpackOverrides] = useState({});

    // Manual / local service fields
    const [localPath, setLocalPath] = useState('');
    const [composeFile, setComposeFile] = useState('');
    const [systemdUnit, setSystemdUnit] = useState('');
    const [managedBy, setManagedBy] = useState('auto');

    // Upload fields
    const [uploadFile, setUploadFile] = useState(null);
    const [uploadDragOver, setUploadDragOver] = useState(false);

    // Optional Project / Environment assignment (opt-in hierarchy).
    const [projects, setProjects] = useState([]);
    const [selectedProjectId, setSelectedProjectId] = useState('');
    const [selectedEnvironmentId, setSelectedEnvironmentId] = useState('');
    const [projectEnvironments, setProjectEnvironments] = useState([]);

    const githubConnection = githubStatus?.connection;
    const githubConfigured = githubStatus?.configured;
    const normalizedManualRepo = useMemo(() => normalizeManualRepo(manualRepoUrl), [manualRepoUrl]);
    const activeManifest = sourceMode === 'template' ? selectedTemplate?.manifest : repoManifest;
    const recommended = activeManifest?.recommended || {};
    const detectedServiceName = useMemo(() => {
        if (sourceMode === 'template' && selectedTemplate) return slugify(selectedTemplate.serviceName || selectedTemplate.name || '');
        if (sourceMode === 'github' && selectedRepo) return slugify(selectedRepo.name || '');
        return repoNameFromUrl(normalizedManualRepo);
    }, [normalizedManualRepo, selectedRepo, selectedTemplate, sourceMode]);
    const serviceName = nameTouched ? name : detectedServiceName;
    const canSubmit = sourceMode === 'github'
        ? Boolean(githubConnection && selectedRepo && serviceName?.length >= 2)
        : sourceMode === 'template'
            ? Boolean(selectedTemplate && serviceName?.length >= 2)
            : sourceMode === 'local'
                ? Boolean(serviceName?.length >= 2 && localPath?.length >= 1)
                : sourceMode === 'upload'
                    ? Boolean(serviceName?.length >= 2 && uploadFile)
                    : Boolean(normalizedManualRepo && serviceName?.length >= 2);
    const buildSummary = buildMethod === 'auto' && recommended.build_method
        ? `Auto -> ${formatBuildMethod(recommended.build_method)}`
        : formatBuildMethod(buildMethod);

    // A managed proxy stack only routes container-based services. The "Proxy
    // stack" ingress option is offered when the selected type resolves to a
    // container: Docker explicitly, Auto-detect (which may resolve to a
    // container), or a Compose-managed local service. Every other type
    // (PHP/Python/static) is served by host Nginx and the backend forces it.
    const ingressProxyEligible = appType === 'docker'
        || appType === 'auto'
        || (sourceMode === 'local' && managedBy === 'docker_compose');

    const loadGithubStatus = useCallback(async () => {
        try {
            const data = await api.getGithubSourceStatus();
            setGithubStatus(data);
        } catch (err) {
            toast.error(err.message || 'Failed to load GitHub connection');
        }
    }, [toast]);

    const loadGithubRepos = useCallback(async (search = '') => {
        setReposLoading(true);
        try {
            const data = await api.listGithubRepositories({ search, perPage: 80 });
            setRepos(data.repos || []);
        } catch (err) {
            toast.error(err.message || 'Failed to load GitHub repositories');
        } finally {
            setReposLoading(false);
        }
    }, [toast]);

    const loadBranches = useCallback(async (fullName) => {
        setBranchesLoading(true);
        try {
            const data = await api.listGithubBranches(fullName);
            setBranches(data.branches || []);
        } catch (err) {
            setBranches([]);
            toast.error(err.message || 'Failed to load branches');
        } finally {
            setBranchesLoading(false);
        }
    }, [toast]);

    useEffect(() => {
        loadGithubStatus();
    }, [loadGithubStatus]);

    // Keep the ingress choice valid: if the selected type is no longer
    // proxy-eligible, snap back to host Nginx (the forced default).
    useEffect(() => {
        if (!ingressProxyEligible && ingressPlane !== 'nginx') {
            setIngressPlane('nginx');
        }
    }, [ingressProxyEligible, ingressPlane]);

    // Load projects for the optional Project / Environment selector. Best-effort:
    // if it fails the selector simply stays empty and creation is unaffected.
    useEffect(() => {
        let cancelled = false;
        api.getProjects()
            .then(data => {
                if (cancelled) return;
                setProjects(Array.isArray(data?.projects) ? data.projects : []);
            })
            .catch(() => { if (!cancelled) setProjects([]); });
        return () => { cancelled = true; };
    }, []);

    // When a project is picked, load its environments and default-select the
    // default one. Clearing the project clears the environment.
    useEffect(() => {
        if (!selectedProjectId) {
            setProjectEnvironments([]);
            setSelectedEnvironmentId('');
            return undefined;
        }
        let cancelled = false;
        api.getProject(selectedProjectId)
            .then(data => {
                if (cancelled) return;
                const envs = Array.isArray(data?.project?.environments) ? data.project.environments : [];
                setProjectEnvironments(envs);
                const def = envs.find(e => e.is_default) || envs[0];
                setSelectedEnvironmentId(def ? String(def.id) : '');
            })
            .catch(() => {
                if (!cancelled) {
                    setProjectEnvironments([]);
                    setSelectedEnvironmentId('');
                }
            });
        return () => { cancelled = true; };
    }, [selectedProjectId]);

    useEffect(() => {
        if (sourceMode === 'github' && githubConnection) {
            loadGithubRepos();
        }
    }, [sourceMode, githubConnection, loadGithubRepos]);

    useEffect(() => {
        if (selectedRepo) {
            setBranch(selectedRepo.default_branch || 'main');
            loadBranches(selectedRepo.full_name);
        }
    }, [selectedRepo, loadBranches]);

    useEffect(() => {
        if (sourceMode !== 'github' || !selectedRepo) {
            if (sourceMode !== 'template') setRepoManifest(null);
            setRepoManifestLoading(false);
            return undefined;
        }

        let cancelled = false;
        setRepoManifestLoading(true);
        api.inspectGithubRepositoryManifest(selectedRepo.full_name, branch || selectedRepo.default_branch || 'main')
            .then((data) => {
                if (cancelled) return;
                const manifest = data.manifest || null;
                setRepoManifest(manifest);
                const detectedPort = manifest?.recommended?.port;
                if (!port && detectedPort) {
                    setPort(String(detectedPort));
                }
            })
            .catch((err) => {
                if (!cancelled) {
                    setRepoManifest(null);
                    toast.error(err.message || 'Failed to inspect repository manifests');
                }
            })
            .finally(() => {
                if (!cancelled) setRepoManifestLoading(false);
            });

        return () => {
            cancelled = true;
        };
    }, [branch, port, selectedRepo, sourceMode, toast]);

    useEffect(() => {
        if (selectedRepo) {
            if (!nameTouched) {
                setName(slugify(selectedRepo.name || ''));
            }
        }
    }, [selectedRepo, nameTouched]);

    // Build-pack detection. Only repo-based sources (github/manual/template) and
    // only when the build method routes through the build pack. Resets overrides
    // whenever the underlying repository changes.
    const buildpackEligible = (buildMethod === 'auto' || buildMethod === 'nixpacks')
        && (sourceMode === 'github' || sourceMode === 'manual' || sourceMode === 'template');

    useEffect(() => {
        if (!buildpackEligible) {
            setBuildpack(null);
            setBuildpackLoading(false);
            return undefined;
        }

        const body = { branch: branch || 'main', name: detectedServiceName || 'app' };
        if (sourceMode === 'github' && selectedRepo && githubConnection) {
            body.source_connection_id = githubConnection.id;
            body.repository_full_name = selectedRepo.full_name;
            body.repo_url = `https://github.com/${selectedRepo.full_name}.git`;
        } else if (sourceMode === 'template' && selectedTemplate) {
            body.repo_url = selectedTemplate.repoUrl;
        } else if (sourceMode === 'manual' && normalizedManualRepo) {
            body.repo_url = normalizedManualRepo;
        } else {
            setBuildpack(null);
            return undefined;
        }

        let cancelled = false;
        setBuildpackLoading(true);
        setBuildpackOverrides({});
        api.detectBuildpack(body)
            .then((data) => {
                if (!cancelled) setBuildpack(data);
            })
            .catch(() => {
                // Detection is best-effort; the user can still pick a method.
                if (!cancelled) setBuildpack(null);
            })
            .finally(() => {
                if (!cancelled) setBuildpackLoading(false);
            });

        return () => { cancelled = true; };
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [buildpackEligible, sourceMode, selectedRepo, selectedTemplate, normalizedManualRepo, branch, githubConnection]);

    async function handleConnectGithub() {
        try {
            const redirectUri = `${window.location.origin}/connections/callback/github`;
            sessionStorage.setItem('sourceConnectionReturnTo', '/services/new');
            const { auth_url } = await api.startSourceConnection('github', redirectUri);
            window.location.href = auth_url;
        } catch (err) {
            toast.error(err.message || 'Failed to start GitHub connection');
        }
    }

    function handleSourceModeChange(mode) {
        setSourceMode(mode);
        if (mode === 'template' && !selectedTemplate) {
            handleSelectTemplate(SERVICE_TEMPLATES[0]);
        }
        if (mode === 'local') {
            setAppType('docker');
        }
        if (mode === 'upload') {
            setAppType('auto');
        }
    }

    function handleSelectTemplate(template) {
        setSelectedTemplate(template);
        setSelectedRepo(null);
        setRepoManifest(template.manifest || null);
        setManualRepoUrl(template.repoUrl);
        setName(slugify(template.serviceName || template.name || ''));
        setNameTouched(false);
        setBranch(template.branch || 'main');
        setAppType(template.appType || 'auto');
        setBuildMethod(template.buildMethod || 'auto');
        setPort(template.port ? String(template.port) : '');
        setAutoDeploy(template.autoDeploy ?? true);
    }

    function handleManualRepoChange(value) {
        setManualRepoUrl(value);
        if (!nameTouched) {
            setName(repoNameFromUrl(normalizeManualRepo(value)));
        }
    }

    async function handleSubmit(e) {
        e.preventDefault();
        if (!canSubmit) {
            const msg = sourceMode === 'github'
                ? 'Select a GitHub repository'
                : sourceMode === 'template'
                    ? 'Select a service template'
                    : sourceMode === 'local'
                        ? 'Service name and server path are required'
                        : sourceMode === 'upload'
                            ? 'Service name and a zip file are required'
                            : 'Repository URL is required';
            toast.error(msg);
            return;
        }

        setSubmitting(true);
        // Optional Project / Environment assignment, included only when picked.
        const projectEnvPayload = {};
        if (selectedProjectId) {
            projectEnvPayload.project_id = Number(selectedProjectId);
            if (selectedEnvironmentId) projectEnvPayload.environment_id = Number(selectedEnvironmentId);
        }
        try {
            if (sourceMode === 'local') {
                const payload = {
                    name: serviceName,
                    app_type: appType,
                    root_path: localPath.trim(),
                    compose_file: composeFile.trim() || undefined,
                    systemd_unit: systemdUnit.trim() || undefined,
                    managed_by: managedBy === 'auto' ? undefined : managedBy,
                    ingress_plane: ingressProxyEligible ? ingressPlane : 'nginx',
                    ...projectEnvPayload,
                };
                const result = await api.createManualApp(payload);
                toast.success('Manual service registered');
                navigate(`/services/${result.app.id}`);
            } else if (sourceMode === 'upload') {
                const formData = new FormData();
                formData.append('file', uploadFile);
                formData.append('name', serviceName);
                formData.append('app_type', appType);
                formData.append('auto_deploy', autoDeploy ? 'true' : 'false');
                formData.append('ingress_plane', ingressProxyEligible ? ingressPlane : 'nginx');
                if (projectEnvPayload.project_id) formData.append('project_id', projectEnvPayload.project_id);
                if (projectEnvPayload.environment_id) formData.append('environment_id', projectEnvPayload.environment_id);
                const result = await api.uploadAppZip(formData);
                toast.success('Upload service created');
                navigate(`/services/${result.app.id}`);
            } else {
                const payload = {
                    name: serviceName,
                    branch: branch.trim() || null,
                    app_type: appType,
                    build_method: buildMethod,
                    port: port ? Number(port) : null,
                    auto_deploy: autoDeploy,
                    ingress_plane: ingressProxyEligible ? ingressPlane : 'nginx',
                    ...projectEnvPayload,
                };
                if (recommended.dockerfile_path) payload.dockerfile_path = recommended.dockerfile_path;
                if (recommended.custom_build_cmd) payload.custom_build_cmd = recommended.custom_build_cmd;
                if (recommended.custom_start_cmd) payload.custom_start_cmd = recommended.custom_start_cmd;
                if (buildpackEligible && buildpack?.plan) {
                    payload.buildpack_plan = buildpack.plan;
                    if (Object.keys(buildpackOverrides).length > 0) {
                        payload.buildpack_overrides = buildpackOverrides;
                    }
                }

                if (sourceMode === 'github') {
                    payload.source_connection_id = githubConnection.id;
                    payload.repository_full_name = selectedRepo.full_name;
                    payload.repo_url = `https://github.com/${selectedRepo.full_name}.git`;
                } else if (sourceMode === 'template') {
                    payload.template_id = selectedTemplate.id;
                    payload.repo_url = selectedTemplate.repoUrl;
                } else {
                    payload.repo_url = normalizedManualRepo;
                }

                const result = await api.createAppFromRepository(payload);
                toast.success('Repository service created');
                navigate(`/services/${result.app.id}`);
            }
        } catch (err) {
            toast.error(err.message || 'Failed to create service');
        } finally {
            setSubmitting(false);
        }
    }

    useTopbarActions(() =>
        <Button type="button" variant="outline" size="sm" asChild>
            <Link to="/settings/connections">
                <Link2 size={16} />
                Connections
            </Link>
        </Button>,
        []
    );

    return (
        <div className="sk-tabgroup__inner new-service-page">
            <div className="new-service-page__method-grid" aria-label="Service source options">
                <button
                    className={`new-service-page__method-card ${sourceMode === 'github' ? 'new-service-page__method-card--on' : ''}`}
                    type="button"
                    onClick={() => handleSourceModeChange('github')}
                >
                    <span className="new-service-page__method-icon">
                        <SiGithub size={21} />
                    </span>
                    <span className="new-service-page__method-title">GitHub</span>
                    <span className="new-service-page__method-sub">Connect with OAuth and choose a repository</span>
                </button>
                <button
                    className={`new-service-page__method-card ${sourceMode === 'manual' ? 'new-service-page__method-card--on' : ''}`}
                    type="button"
                    onClick={() => handleSourceModeChange('manual')}
                >
                    <span className="new-service-page__method-icon">
                        <GitBranch size={21} />
                    </span>
                    <span className="new-service-page__method-title">Other Git Remote</span>
                    <span className="new-service-page__method-sub">GitLab, Bitbucket, Gitea, or SSH</span>
                </button>
                <button
                    className={`new-service-page__method-card ${sourceMode === 'local' ? 'new-service-page__method-card--on' : ''}`}
                    type="button"
                    onClick={() => handleSourceModeChange('local')}
                >
                    <span className="new-service-page__method-icon">
                        <FolderOpen size={21} />
                    </span>
                    <span className="new-service-page__method-title">Manual / Local</span>
                    <span className="new-service-page__method-sub">Register an app that already exists on the server</span>
                </button>
                <button
                    className={`new-service-page__method-card ${sourceMode === 'upload' ? 'new-service-page__method-card--on' : ''}`}
                    type="button"
                    onClick={() => handleSourceModeChange('upload')}
                >
                    <span className="new-service-page__method-icon">
                        <FileArchive size={21} />
                    </span>
                    <span className="new-service-page__method-title">Upload ZIP</span>
                    <span className="new-service-page__method-sub">Deploy or update from a zip archive</span>
                </button>
                <button
                    className={`new-service-page__method-card ${sourceMode === 'template' ? 'new-service-page__method-card--on' : ''}`}
                    type="button"
                    onClick={() => handleSourceModeChange('template')}
                >
                    <span className="new-service-page__method-icon">
                        <Package size={21} />
                    </span>
                    <span className="new-service-page__method-title">Deploy Template</span>
                    <span className="new-service-page__method-sub">Fast import from manifest-ready repos</span>
                </button>
            </div>

            <form className="new-service-page__wizard" onSubmit={handleSubmit}>
                <section className="new-service-page__panel new-service-page__provider-panel">
                    <div className="new-service-page__section-heading">
                        <Link2 size={16} />
                        <h2>
                            {sourceMode === 'github'
                                ? 'Pick Repository'
                                : sourceMode === 'template'
                                    ? 'Choose Template'
                                    : sourceMode === 'local'
                                        ? 'Local Service'
                                        : sourceMode === 'upload'
                                            ? 'Upload Archive'
                                            : 'Connect Source'}
                        </h2>
                    </div>

                    {sourceMode === 'github' ? (
                        <div className="new-service-page__connect-box">
                            {githubConnection ? (
                                <>
                                    <div className="new-service-page__github-account">
                                        {githubConnection.avatar_url && <img src={githubConnection.avatar_url} alt="" />}
                                        <div>
                                            <strong>{githubConnection.display_name || githubConnection.provider_username}</strong>
                                            <span>@{githubConnection.provider_username}</span>
                                        </div>
                                        <Button type="button" variant="outline" onClick={() => loadGithubRepos()}>
                                            <RefreshCw size={16} className={reposLoading ? 'spinning' : ''} />
                                            Refresh
                                        </Button>
                                    </div>

                                    <div className="new-service-page__repo-search">
                                        <Search size={16} />
                                        <Input
                                            value={repoSearch}
                                            onChange={(e) => setRepoSearch(e.target.value)}
                                            placeholder="Search repositories"
                                        />
                                        <Button type="button" variant="outline" onClick={() => loadGithubRepos(repoSearch)}>
                                            Search
                                        </Button>
                                    </div>

                                    <div className="new-service-page__repo-list">
                                        {reposLoading && <div className="new-service-page__repo-state">Loading repositories...</div>}
                                        {!reposLoading && repos.length === 0 && (
                                            <div className="new-service-page__repo-state">No repositories found.</div>
                                        )}
                                        {!reposLoading && repos.map(repo => (
                                            <button
                                                key={repo.id}
                                                type="button"
                                                className={`new-service-page__repo-row ${selectedRepo?.id === repo.id ? 'new-service-page__repo-row--active' : ''}`}
                                                onClick={() => setSelectedRepo(repo)}
                                            >
                                                <span>
                                                    <strong>{repo.full_name}</strong>
                                                    <small>{repo.description || repo.language || 'No description'}</small>
                                                </span>
                                                <em>{repo.private ? 'Private' : 'Public'}</em>
                                            </button>
                                        ))}
                                    </div>
                                </>
                            ) : (
                                <div className="new-service-page__connect-empty">
                                    <span className="new-service-page__connect-icon">
                                        <SiGithub size={20} />
                                    </span>
                                    <div>
                                        <h2>{githubConfigured ? 'Connect GitHub' : 'GitHub connection is not configured'}</h2>
                                        <p>
                                            {githubConfigured
                                                ? 'Authorize ServerKit once, then choose a repository from your GitHub account.'
                                                : 'Add the GitHub OAuth app credentials in Settings before connecting.'}
                                        </p>
                                    </div>
                                    <div className="new-service-page__connect-actions">
                                        <Button type="button" onClick={handleConnectGithub} disabled={!githubConfigured}>
                                            <SiGithub size={16} />
                                            Connect GitHub
                                        </Button>
                                        <Button type="button" variant="outline" asChild>
                                            <Link to="/settings/connections">
                                                <Settings2 size={16} />
                                                Settings
                                            </Link>
                                        </Button>
                                    </div>
                                </div>
                            )}
                        </div>
                    ) : sourceMode === 'template' ? (
                        <div className="new-service-page__connect-box">
                            <div className="new-service-page__connect-heading">
                                <span className="new-service-page__connect-icon">
                                    <Package size={18} />
                                </span>
                                <div>
                                    <strong>Manifest-ready templates</strong>
                                    <span>Templates can ship Render, Railway, Docker Compose, app.json, or ServerKit manifest files.</span>
                                </div>
                            </div>

                            <div className="new-service-page__template-list">
                                {SERVICE_TEMPLATES.map(template => (
                                    <button
                                        key={template.id}
                                        type="button"
                                        className={`new-service-page__template-row ${selectedTemplate?.id === template.id ? 'new-service-page__template-row--active' : ''}`}
                                        onClick={() => handleSelectTemplate(template)}
                                    >
                                        <span className="new-service-page__template-main">
                                            <strong>{template.name}</strong>
                                            <small>{template.description}</small>
                                            <em>{template.repoUrl}</em>
                                        </span>
                                        <span className="new-service-page__template-badges">
                                            {template.badges.map(badge => (
                                                <span key={badge}>{badge}</span>
                                            ))}
                                        </span>
                                        {selectedTemplate?.id === template.id ? <CheckCircle2 size={18} /> : <ArrowRight size={18} />}
                                    </button>
                                ))}
                            </div>

                            <div className="new-service-page__connect-actions new-service-page__connect-actions--left">
                                <Button type="button" variant="outline" asChild>
                                    <Link to="/templates">
                                        <Package size={16} />
                                        Template Library
                                    </Link>
                                </Button>
                            </div>
                        </div>
                    ) : sourceMode === 'local' ? (
                        <div className="new-service-page__connect-box">
                            <div className="new-service-page__connect-heading">
                                <span className="new-service-page__connect-icon">
                                    <FolderOpen size={18} />
                                </span>
                                <div>
                                    <strong>Register an existing service</strong>
                                    <span>Point ServerKit at a directory or systemd unit that is already on the server.</span>
                                </div>
                            </div>
                            <div className="new-service-page__field">
                                <Label htmlFor="local-path">Path on server</Label>
                                <Input
                                    id="local-path"
                                    value={localPath}
                                    onChange={(e) => setLocalPath(e.target.value)}
                                    placeholder="/opt/my-service"
                                    autoComplete="off"
                                    required={sourceMode === 'local'}
                                />
                            </div>
                            <div className="new-service-page__field">
                                <Label htmlFor="compose-file">Compose file (optional)</Label>
                                <Input
                                    id="compose-file"
                                    value={composeFile}
                                    onChange={(e) => setComposeFile(e.target.value)}
                                    placeholder="docker-compose.yml"
                                    autoComplete="off"
                                />
                            </div>
                            <div className="new-service-page__field">
                                <Label htmlFor="systemd-unit">systemd unit (optional)</Label>
                                <Input
                                    id="systemd-unit"
                                    value={systemdUnit}
                                    onChange={(e) => setSystemdUnit(e.target.value)}
                                    placeholder="my-service"
                                    autoComplete="off"
                                />
                            </div>
                            <div className="new-service-page__field">
                                <Label htmlFor="managed-by">Managed by</Label>
                                <select
                                    id="managed-by"
                                    value={managedBy}
                                    onChange={(e) => setManagedBy(e.target.value)}
                                >
                                    <option value="auto">Auto-detect</option>
                                    <option value="docker_compose">Docker Compose</option>
                                    <option value="systemd">systemd</option>
                                </select>
                            </div>
                        </div>
                    ) : sourceMode === 'upload' ? (
                        <div className="new-service-page__connect-box">
                            <div className="new-service-page__connect-heading">
                                <span className="new-service-page__connect-icon">
                                    <FileArchive size={18} />
                                </span>
                                <div>
                                    <strong>Upload a zip archive</strong>
                                    <span>ServerKit will extract the archive, detect the runtime, and deploy it for you.</span>
                                </div>
                            </div>
                            <div
                                className={`new-service-page__upload-drop ${uploadDragOver ? 'new-service-page__upload-drop--over' : ''}`}
                                onDragOver={(e) => { e.preventDefault(); setUploadDragOver(true); }}
                                onDragLeave={() => setUploadDragOver(false)}
                                onDrop={(e) => {
                                    e.preventDefault();
                                    setUploadDragOver(false);
                                    const file = e.dataTransfer.files[0];
                                    if (file) {
                                        setUploadFile(file);
                                        if (!nameTouched) {
                                            setName(slugify(file.name.replace(/\.zip$/i, '')));
                                        }
                                    }
                                }}
                                onClick={() => document.getElementById('upload-zip')?.click()}
                            >
                                <FileArchive size={32} />
                                <span>
                                    {uploadFile
                                        ? uploadFile.name
                                        : 'Drag a zip here or click to browse'}
                                </span>
                                <input
                                    id="upload-zip"
                                    type="file"
                                    accept=".zip,application/zip,application/x-zip-compressed"
                                    className="sr-only"
                                    onChange={(e) => {
                                        const file = e.target.files[0];
                                        if (file) {
                                            setUploadFile(file);
                                            if (!nameTouched) {
                                                setName(slugify(file.name.replace(/\.zip$/i, '')));
                                            }
                                        }
                                    }}
                                />
                            </div>
                        </div>
                    ) : (
                        <div className="new-service-page__connect-box">
                            <div className="new-service-page__connect-heading">
                                <span className="new-service-page__connect-icon">
                                    <GitBranch size={18} />
                                </span>
                                <div>
                                    <strong>Git remote</strong>
                                    <span>Use this for providers that are not connected through the GitHub API.</span>
                                </div>
                            </div>
                            <div className="new-service-page__field">
                                <Label htmlFor="manual-repo-url">Repository URL</Label>
                                <Input
                                    id="manual-repo-url"
                                    value={manualRepoUrl}
                                    onChange={(e) => handleManualRepoChange(e.target.value)}
                                    placeholder="git@gitea.example.com:owner/repo.git"
                                    autoComplete="off"
                                    required={sourceMode === 'manual'}
                                />
                            </div>
                        </div>
                    )}

                    {(selectedRepo || sourceMode === 'manual' || sourceMode === 'local' || sourceMode === 'upload' || selectedTemplate) && (
                        <div className="new-service-page__repo-preview">
                            <div>
                                <span>Service</span>
                                <strong>{serviceName || 'Auto-named'}</strong>
                            </div>
                            {sourceMode !== 'local' && sourceMode !== 'upload' && (
                                <div>
                                    <span>Branch</span>
                                    <strong>{branch || 'main'}</strong>
                                </div>
                            )}
                            {sourceMode === 'local' && (
                                <div>
                                    <span>Path</span>
                                    <strong>{localPath || '—'}</strong>
                                </div>
                            )}
                            <div>
                                <span>Type</span>
                                <strong>{sourceMode === 'upload' && appType === 'auto' ? 'Auto-detect' : formatAppType(appType)}</strong>
                            </div>
                        </div>
                    )}
                </section>

                <aside className="new-service-page__panel new-service-page__review-panel">
                    <div className="new-service-page__deploy-card">
                        <span className="new-service-page__deploy-icon">
                            <Rocket size={18} />
                        </span>
                        <div>
                            <h2>
                                {sourceMode === 'local'
                                    ? 'Register Local Service'
                                    : sourceMode === 'upload'
                                        ? 'Deploy from Archive'
                                        : 'Ready to Import'}
                            </h2>
                            <p>
                                {sourceMode === 'local'
                                    ? 'ServerKit will link to the existing path or systemd unit and poll its real status.'
                                    : sourceMode === 'upload'
                                        ? 'ServerKit will extract the archive, detect the runtime, and deploy the service.'
                                        : 'ServerKit clones the selected repository, detects the runtime, configures builds, and records deployment settings.'}
                            </p>
                        </div>
                    </div>

                    <div className="new-service-page__flow">
                        <div>
                            {sourceMode === 'local' ? <FolderOpen size={16} /> : sourceMode === 'upload' ? <FileArchive size={16} /> : <SiGithub size={16} />}
                            <span>{sourceMode === 'local' ? 'Path' : sourceMode === 'upload' ? 'Upload' : 'Connect'}</span>
                        </div>
                        <ArrowRight size={14} />
                        <div>
                            <Zap size={16} />
                            <span>Detect</span>
                        </div>
                        <ArrowRight size={14} />
                        <div>
                            <Server size={16} />
                            <span>Deploy</span>
                        </div>
                    </div>

                    {(repoManifestLoading || activeManifest) && (
                        <div className="new-service-page__manifest-card">
                            <div className="new-service-page__manifest-head">
                                <span>
                                    <Zap size={16} />
                                    Manifest Detection
                                </span>
                                <strong>{repoManifestLoading ? 'Inspecting' : activeManifest?.strategy?.replace('_', ' ') || 'Detected'}</strong>
                            </div>
                            {!repoManifestLoading && activeManifest && (
                                <>
                                    <div className="new-service-page__manifest-grid">
                                        <div>
                                            <span>Type</span>
                                            <strong>{formatAppType(recommended.app_type)}</strong>
                                        </div>
                                        <div>
                                            <span>Build</span>
                                            <strong>{formatBuildMethod(recommended.build_method)}</strong>
                                        </div>
                                        <div>
                                            <span>Port</span>
                                            <strong>{recommended.port || 'Auto'}</strong>
                                        </div>
                                    </div>
                                    <div className="new-service-page__manifest-files">
                                        {(activeManifest.manifests || []).slice(0, 5).map(manifest => (
                                            <span key={manifest.file}>
                                                <CheckCircle2 size={13} />
                                                {manifest.file}
                                            </span>
                                        ))}
                                    </div>
                                    {(activeManifest.env || []).length > 0 && (
                                        <div className="new-service-page__env-preview">
                                            {(activeManifest.env || []).slice(0, 6).map(env => (
                                                <span key={env.key} className={env.secret ? 'new-service-page__env-preview-secret' : ''}>
                                                    {env.key}
                                                </span>
                                            ))}
                                        </div>
                                    )}
                                </>
                            )}
                        </div>
                    )}

                    {buildpackEligible && (buildpackLoading || buildpack?.plan) && (
                        <BuildpackPreview
                            plan={buildpack?.plan}
                            dockerfile={buildpack?.dockerfile}
                            overrides={buildpackOverrides}
                            onChange={setBuildpackOverrides}
                            loading={buildpackLoading}
                        />
                    )}

                    <div className="new-service-page__summary">
                        <div>
                            <span>Source</span>
                            <strong>
                                {sourceMode === 'github'
                                    ? 'GitHub API'
                                    : sourceMode === 'template'
                                        ? 'Template'
                                        : sourceMode === 'local'
                                            ? 'Manual / Local'
                                            : sourceMode === 'upload'
                                                ? 'Upload'
                                                : 'Git remote'}
                            </strong>
                        </div>
                        <div>
                            <span>{sourceMode === 'local' ? 'Path' : sourceMode === 'upload' ? 'Archive' : 'Repository'}</span>
                            <strong>
                                {sourceMode === 'local'
                                    ? localPath || 'Not set'
                                    : sourceMode === 'upload'
                                        ? uploadFile?.name || 'Not selected'
                                        : selectedRepo?.full_name || selectedTemplate?.repoUrl || normalizedManualRepo || 'Not selected'}
                            </strong>
                        </div>
                        <div>
                            <span>{sourceMode === 'upload' ? 'Detection' : 'Build'}</span>
                            <strong>{sourceMode === 'upload' ? (appType === 'auto' ? 'Auto-detect' : formatAppType(appType)) : buildSummary}</strong>
                        </div>
                        {sourceMode !== 'local' && (
                            <div>
                                <span>Auto-deploy</span>
                                <strong>{autoDeploy ? 'On' : 'Off'}</strong>
                            </div>
                        )}
                        <div>
                            <span>Ingress</span>
                            <strong>
                                {ingressProxyEligible && ingressPlane === 'proxy_stack'
                                    ? 'Proxy stack'
                                    : 'Host Nginx'}
                            </strong>
                        </div>
                    </div>

                    <button
                        className="new-service-page__advanced-toggle"
                        type="button"
                        onClick={() => setAdvancedOpen(open => !open)}
                        aria-expanded={advancedOpen}
                    >
                        <span>
                            <Settings2 size={16} />
                            Advanced settings
                        </span>
                        <ChevronDown size={16} />
                    </button>

                    {advancedOpen && (
                        <div className="new-service-page__advanced">
                            <div className="new-service-page__two-col">
                                {sourceMode !== 'local' && sourceMode !== 'upload' && (
                                    <div className="new-service-page__field">
                                        <Label htmlFor="branch">Branch</Label>
                                        {sourceMode === 'github' && branches.length > 0 ? (
                                            <select
                                                id="branch"
                                                value={branch}
                                                onChange={(e) => setBranch(e.target.value)}
                                                disabled={branchesLoading}
                                            >
                                                {branches.map(option => (
                                                    <option key={option.name} value={option.name}>{option.name}</option>
                                                ))}
                                            </select>
                                        ) : (
                                            <Input
                                                id="branch"
                                                value={branch}
                                                onChange={(e) => setBranch(e.target.value)}
                                                placeholder="main"
                                            />
                                        )}
                                    </div>
                                )}
                                <div className="new-service-page__field">
                                    <Label htmlFor="service-name">Service name</Label>
                                    <Input
                                        id="service-name"
                                        value={serviceName}
                                        onChange={(e) => {
                                            setNameTouched(true);
                                            setName(slugify(e.target.value));
                                        }}
                                        placeholder="my-service"
                                        minLength={2}
                                        required
                                    />
                                </div>
                            </div>

                            <div className="new-service-page__two-col">
                                <div className="new-service-page__field">
                                    <Label htmlFor="app-type">Service type</Label>
                                    <select
                                        id="app-type"
                                        value={appType}
                                        onChange={(e) => setAppType(e.target.value)}
                                    >
                                        {sourceMode === 'upload' && <option value="auto">Auto-detect</option>}
                                        {APP_TYPE_OPTIONS.filter(o => o.value !== 'auto').map(option => (
                                            <option key={option.value} value={option.value}>{option.label}</option>
                                        ))}
                                    </select>
                                </div>
                                {sourceMode !== 'local' && sourceMode !== 'upload' && (
                                    <div className="new-service-page__field">
                                        <Label htmlFor="build-method">Build method</Label>
                                        <select
                                            id="build-method"
                                            value={buildMethod}
                                            onChange={(e) => setBuildMethod(e.target.value)}
                                        >
                                            {BUILD_METHOD_OPTIONS.map(option => (
                                                <option key={option.value} value={option.value}>{option.label}</option>
                                            ))}
                                        </select>
                                    </div>
                                )}
                            </div>

                            <div className="new-service-page__two-col">
                                <div className="new-service-page__field">
                                    <Label htmlFor="port">Runtime port</Label>
                                    <Input
                                        id="port"
                                        type="number"
                                        value={port}
                                        onChange={(e) => setPort(e.target.value)}
                                        placeholder="3000"
                                        min="1"
                                        max="65535"
                                    />
                                </div>
                                {sourceMode !== 'local' && (
                                    <div className="new-service-page__toggle">
                                        <div>
                                            <Label>Auto-deploy</Label>
                                            <span>{sourceMode === 'upload' ? 'Deploy immediately after upload.' : 'Webhook deployment for this branch.'}</span>
                                        </div>
                                        <Switch checked={autoDeploy} onCheckedChange={setAutoDeploy} />
                                    </div>
                                )}
                            </div>

                            <div className="new-service-page__field">
                                <Label htmlFor="ingress-plane">Ingress</Label>
                                {ingressProxyEligible ? (
                                    <select
                                        id="ingress-plane"
                                        value={ingressPlane}
                                        onChange={(e) => setIngressPlane(e.target.value)}
                                    >
                                        <option value="nginx">Host Nginx (default)</option>
                                        <option value="proxy_stack">Proxy stack (Traefik / Caddy)</option>
                                    </select>
                                ) : (
                                    <div className="new-service-page__note">
                                        <Network size={16} />
                                        <span>Served by host Nginx</span>
                                    </div>
                                )}
                            </div>

                            {projects.length > 0 && (
                                <div className="new-service-page__two-col">
                                    <div className="new-service-page__field">
                                        <Label htmlFor="project">Project <span className="new-service-page__optional">(optional)</span></Label>
                                        <select
                                            id="project"
                                            value={selectedProjectId}
                                            onChange={(e) => setSelectedProjectId(e.target.value)}
                                        >
                                            <option value="">No project</option>
                                            {projects.map(p => (
                                                <option key={p.id} value={p.id}>{p.name}</option>
                                            ))}
                                        </select>
                                    </div>
                                    {selectedProjectId && (
                                        <div className="new-service-page__field">
                                            <Label htmlFor="environment">Environment</Label>
                                            <select
                                                id="environment"
                                                value={selectedEnvironmentId}
                                                onChange={(e) => setSelectedEnvironmentId(e.target.value)}
                                            >
                                                {projectEnvironments.map(env => (
                                                    <option key={env.id} value={env.id}>{env.name}</option>
                                                ))}
                                            </select>
                                        </div>
                                    )}
                                </div>
                            )}
                        </div>
                    )}

                    <div className="new-service-page__notes">
                        <div className="new-service-page__note">
                            <ShieldCheck size={16} />
                            <span>
                                {sourceMode === 'local'
                                    ? 'ServerKit will read status from Docker Compose or systemd without modifying the original path.'
                                    : sourceMode === 'upload'
                                        ? 'Uploaded archives are versioned, so you can roll back to a previous release.'
                                        : 'ServerKit checks serverkit.json, Docker Compose, Render, Railway, app.json, Dockerfile, and Nixpacks signals.'}
                            </span>
                        </div>
                        <div className="new-service-page__note">
                            <Lock size={16} />
                            <span>{sourceMode === 'upload' ? 'Existing .env files are preserved when uploading a new version.' : 'Secret values from manifests stay empty until you add them to the service environment.'}</span>
                        </div>
                    </div>

                    <div className="new-service-page__actions">
                        <Button type="button" variant="outline" asChild>
                            <Link to="/services">Cancel</Link>
                        </Button>
                        <Button type="submit" disabled={!canSubmit || submitting}>
                            <Rocket size={16} />
                            {submitting
                                ? (sourceMode === 'local' ? 'Registering...' : sourceMode === 'upload' ? 'Uploading...' : 'Importing...')
                                : (sourceMode === 'local' ? 'Register Service' : sourceMode === 'upload' ? 'Upload & Deploy' : 'Import Repository')}
                        </Button>
                    </div>
                </aside>
            </form>
        </div>
    );
};

export default NewService;
