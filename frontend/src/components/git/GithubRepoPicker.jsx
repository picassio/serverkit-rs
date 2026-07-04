import { useState, useEffect, useCallback } from 'react';
import { SiGithub } from 'react-icons/si';
import { Search, RefreshCw, Check } from 'lucide-react';
import { Link } from 'react-router-dom';
import api from '../../services/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

// One-click "Connect with GitHub" repo picker — the easy alternative to pasting a
// clone URL. It reuses the SAME GitHub source-connection OAuth that the New
// Service page already uses (no new backend), so any connect surface can offer
// "authorize once, then pick a repo". It is purely a selection helper: it calls
// onPick({ repoUrl, fullName, branch }) as the user chooses, and the host form
// turns that into its own connect request.
function cloneUrl(repo) {
    return repo.clone_url || `https://github.com/${repo.full_name}.git`;
}

const GithubRepoPicker = ({ onPick }) => {
    const [status, setStatus] = useState(null); // { configured, connection }
    const [loading, setLoading] = useState(true);
    const [repos, setRepos] = useState([]);
    const [reposLoading, setReposLoading] = useState(false);
    const [search, setSearch] = useState('');
    const [selected, setSelected] = useState(null);
    const [branches, setBranches] = useState([]);
    const [branch, setBranch] = useState('');

    const connection = status?.connection;
    const configured = status?.configured;

    const loadStatus = useCallback(async () => {
        setLoading(true);
        try {
            setStatus(await api.getGithubSourceStatus());
        } catch {
            setStatus(null);
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => { loadStatus(); }, [loadStatus]);

    const loadRepos = useCallback(async (q = '') => {
        setReposLoading(true);
        try {
            const data = await api.listGithubRepositories({ search: q, perPage: 80 });
            setRepos(data.repos || []);
        } catch {
            setRepos([]);
        } finally {
            setReposLoading(false);
        }
    }, []);

    useEffect(() => { if (connection) loadRepos(); }, [connection, loadRepos]);

    // Picking a repo defaults the branch, loads its branch list, and emits.
    useEffect(() => {
        if (!selected) return undefined;
        const b = selected.default_branch || 'main';
        setBranch(b);
        onPick?.({ repoUrl: cloneUrl(selected), fullName: selected.full_name, branch: b });
        let cancelled = false;
        api.listGithubBranches(selected.full_name)
            .then((data) => { if (!cancelled) setBranches(data.branches || []); })
            .catch(() => { if (!cancelled) setBranches([]); });
        return () => { cancelled = true; };
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [selected]);

    function pickBranch(b) {
        setBranch(b);
        if (selected) onPick?.({ repoUrl: cloneUrl(selected), fullName: selected.full_name, branch: b });
    }

    async function connectGithub() {
        try {
            const redirectUri = `${window.location.origin}/connections/callback/github`;
            sessionStorage.setItem('sourceConnectionReturnTo', window.location.pathname + window.location.search);
            const { auth_url } = await api.startSourceConnection('github', redirectUri);
            window.location.href = auth_url;
        } catch {
            /* a retry surfaces the host's toast; nothing to do here */
        }
    }

    if (loading) return null;

    // No OAuth app configured — nudge to Settings, but never block the paste-URL
    // path the host form still renders below this.
    if (!configured) {
        return (
            <div className="git-connect__gh git-connect__gh--hint">
                <SiGithub size={15} aria-hidden="true" />
                <span>
                    Set up the GitHub connection in <Link to="/settings/connections">Settings</Link> to
                    pick a repo in one click instead of pasting a URL.
                </span>
            </div>
        );
    }

    if (!connection) {
        return (
            <div className="git-connect__gh git-connect__gh--connect">
                <SiGithub size={18} aria-hidden="true" />
                <div className="git-connect__gh-text">
                    <strong>Connect with GitHub</strong>
                    <span>Authorize once, then choose a repository instead of pasting a URL.</span>
                </div>
                <Button type="button" onClick={connectGithub}>
                    <SiGithub size={15} /> Connect GitHub
                </Button>
            </div>
        );
    }

    return (
        <div className="git-connect__gh git-connect__gh--picker">
            <div className="git-connect__gh-account">
                {connection.avatar_url && <img src={connection.avatar_url} alt="" />}
                <span className="git-connect__gh-account-name">
                    <strong>{connection.display_name || connection.provider_username}</strong>
                    <small>@{connection.provider_username}</small>
                </span>
                <Button type="button" variant="outline" size="sm" onClick={() => loadRepos(search)}>
                    <RefreshCw size={14} className={reposLoading ? 'spinning' : ''} /> Refresh
                </Button>
            </div>

            <div className="git-connect__gh-search">
                <Search size={15} aria-hidden="true" />
                <Input
                    value={search}
                    onChange={(e) => setSearch(e.target.value)}
                    placeholder="Search your repositories"
                    onKeyDown={(e) => { if (e.key === 'Enter') { e.preventDefault(); loadRepos(search); } }}
                />
            </div>

            <div className="git-connect__gh-list">
                {reposLoading && <div className="git-connect__gh-state">Loading repositories…</div>}
                {!reposLoading && repos.length === 0 && <div className="git-connect__gh-state">No repositories found.</div>}
                {!reposLoading && repos.map((repo) => (
                    <button
                        type="button"
                        key={repo.id}
                        className={`git-connect__gh-repo${selected?.id === repo.id ? ' is-active' : ''}`}
                        onClick={() => setSelected(repo)}
                    >
                        <span className="git-connect__gh-repo-main">
                            <strong>{repo.full_name}</strong>
                            <small>{repo.description || repo.language || 'No description'}</small>
                        </span>
                        <span className="git-connect__gh-repo-vis">{repo.private ? 'Private' : 'Public'}</span>
                        {selected?.id === repo.id && <Check size={15} />}
                    </button>
                ))}
            </div>

            {selected && branches.length > 0 && (
                <div className="git-connect__gh-branch">
                    <label htmlFor="gh-branch">Branch</label>
                    <select id="gh-branch" value={branch} onChange={(e) => pickBranch(e.target.value)}>
                        {branches.map((b) => {
                            const name = typeof b === 'string' ? b : b.name;
                            return <option key={name} value={name}>{name}</option>;
                        })}
                    </select>
                </div>
            )}
        </div>
    );
};

export default GithubRepoPicker;
