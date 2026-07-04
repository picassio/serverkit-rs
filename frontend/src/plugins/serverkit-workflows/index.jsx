// Workflow Builder, contributed through the extension system. The page component
// and its /api/v1/workflows backend stay in core for now (two-speed extraction,
// D2); this extension owns the /workflow route + sidebar/palette entries via its
// manifest. The route uses the "full" layout (no padding) to match the canvas.
//
// After sync this file lives at frontend/src/plugins/serverkit-workflows/ so the
// relative import resolves against the host's pages directory.
import WorkflowBuilder from '../../pages/WorkflowBuilder';

export function WorkflowBuilderPage() {
    return <WorkflowBuilder />;
}
