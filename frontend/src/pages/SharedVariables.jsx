import SharedVariableGroups from '../components/shared/SharedVariableGroups';

/**
 * SharedVariables — workspace-scoped management of shared variable groups
 * (the polymorphic facade: groups of variables that attach to any resource).
 * Scoped to the active workspace from localStorage; falls back to 'default'.
 */
const SharedVariables = () => {
    const workspaceId = localStorage.getItem('active_workspace_id') || 'default';

    return (
        <div className="sk-tabgroup__inner shared-variables-page">
            <SharedVariableGroups scopeType="workspace" scopeId={workspaceId} />
        </div>
    );
};

export default SharedVariables;
