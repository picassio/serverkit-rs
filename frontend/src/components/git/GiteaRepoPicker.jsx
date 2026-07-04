import { useState, useEffect, useCallback } from 'react';
import { SiGitea } from 'react-icons/si';
import { Search, RefreshCw, Check, Server } from 'lucide-react';
import { Link } from 'react-router-dom';
import api from '../../services/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

// Local Gitea repo picker. Uses the ServerKit-managed Gitea instance when it is
// installed and running. The backend's formatted repo payload already includes
// clone_url, so we emit that directly.
function cloneUrl(repo) {
    return repo.clone_url || repo.html_url || repo.ssh_url || '';
}

function splitFullName(fullName) {
    const parts = (fullName || '').split('/');
    return parts.length >= 2 ? [parts[0], parts.slice(1).join('/')] : [null, null];
}

const GiteaRepoPicker = ({ onPick }) => {
    const [status, setStatus] = useState(null);
    const [loading, setLoading] = useState(true);
    const [repos, setRepos] = useState([]);
    const [reposLoading, setReposLoading] = useState(false);
    const [search, setSearch] = useState('');
    const [selected, setSelected] = useState(null);
    const [branches, setBranches] = useState([]);
    const [branch, setBranch] = useState('');

    const installed = status?.installed;
    const running = status?.running;

    const loadStatus = useCallback(async () => {
        setLoading(true);
        try {
            const data = await api.getGiteaStatus();
            setStatus(data);
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
            const data = await api.getGiteaRepositories(200);
            let list = data.repositories || data.repos || [];
            if (q.trim()) {
                const lower = q.trim().toLowerCase();
                list = list.filter((r) =>
                    (r.full_name || r.name || '').toLowerCase().includes(lower)
                );
            }
            setRepos(list);
        } catch {
            setRepos([]);
        } finally {
            setReposLoading(false);
        }
    }, []);

    useEffect(() => { if (installed && running) loadRepos(); }, [installed, running, loadRepos]);

    useEffect(() => {
        if (!selected) return undefined;
        const [owner, repo] = splitFullName(selected.full_name);
        const b = selected.default_branch || 'main';
        setBranch(b);
        onPick?.({ repoUrl: cloneUrl(selected), fullName: selected.full_name, branch: b });
        let cancelled = false;
        if (owner && repo) {
            api.getGiteaBranches(owner, repo)
                .then((data) => { if (!cancelled) setBranches(data.branches || []); })
                .catch(() => { if (!cancelled) setBranches([]); });
        }
        return () => { cancelled = true; };
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [selected]);

    function pickBranch(b) {
        setBranch(b);
        if (selected) onPick?.({ repoUrl: cloneUrl(selected), fullName: selected.full_name, branch: b });
    }

    if (loading) return null;

    if (!installed) {
        return (
            <div className="git-connect__gh git-connect__gh--hint">
                <SiGitea size={15} aria-hidden="true" />
                <span>
                    Install the ServerKit Git server in <Link to="/git">Git</Link> to
                    pick a local repository instead of pasting a URL.
                </span>
            </div>
        );
    }

    if (!running) {
        return (
            <div className="git-connect__gh git-connect__gh--hint">
                <Server size={15} aria-hidden="true" />
                <span>
                    Your Gitea server is installed but not running. Start it from <Link to="/git">Git</Link>.
                </span>
            </div>
        );
    }

    return (
        <div className="git-connect__gh git-connect__gh--picker">
            <div className="git-connect__gh-account">
                <span className="git-connect__gh-account-name">
                    <strong>Local Gitea</strong>
                    <small>Repositories on this ServerKit instance</small>
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
                    placeholder="Search local repositories"
                    onKeyDown={(e) => { if (e.key === 'Enter') { e.preventDefault(); loadRepos(search); } }}
                />
            </div>

            <div className="git-connect__gh-list">
                {reposLoading && <div className="git-connect__gh-state">Loading repositories…</div>}
                {!reposLoading && repos.length === 0 && <div className="git-connect__gh-state">No local repositories found.</div>}
                {!reposLoading && repos.map((repo) => (
                    <button
                        type="button"
                        key={repo.id}
                        className={`git-connect__gh-repo${selected?.id === repo.id ? ' is-active' : ''}`}
                        onClick={() => setSelected(repo)}
                    >
                        <span className="git-connect__gh-repo-main">
                            <strong>{repo.full_name}</strong>
                            <small>{repo.description || 'No description'}</small>
                        </span>
                        <span className="git-connect__gh-repo-vis">{repo.private ? 'Private' : 'Public'}</span>
                        {selected?.id === repo.id && <Check size={15} />}
                    </button>
                ))}
            </div>

            {selected && branches.length > 0 && (
                <div className="git-connect__gh-branch">
                    <label htmlFor="gt-branch">Branch</label>
                    <select id="gt-branch" value={branch} onChange={(e) => pickBranch(e.target.value)}>
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

export default GiteaRepoPicker;
