// WordPress UI, contributed through the extension system. The full WordPress
// backend (the /api/v1/wordpress* blueprints + services) lives in this
// extension's backend/; the page components stay in the host pages/ dir and are
// re-used here (same re-export pattern as serverkit-git/email/workflows) so
// their many relative imports keep resolving.
//
// After sync this file lives at frontend/src/plugins/serverkit-wordpress/ so the
// relative imports resolve against the host's src/ tree.
//
// The WordPress surface is a tab group + full-bleed detail pages. Contributed
// routes are flat (there's no way to contribute a shared TabGroupLayout parent
// inside the dashboard), so we contribute ONE splat route (`wordpress/*`) and
// this component self-renders the whole WordPress sub-router — reproducing the
// exact structure the core App.jsx used to own (TabGroupLayout + WORDPRESS_TABS
// over the three list pages; full-bleed detail/pipeline routes as siblings;
// the legacy /wordpress/projects → /wordpress/pipelines deprecation redirects).
import { Routes, Route, Navigate, useParams } from 'react-router-dom';

import TabGroupLayout from '../../layouts/TabGroupLayout';
import { WORDPRESS_TABS } from '../../components/wordpress/wordpressTabs';
import WordPress from '../../pages/WordPress';
import WordPressPluginLibrary from '../../pages/WordPressPluginLibrary';
import WordPressProjects from '../../pages/WordPressProjects';
import WordPressProject from '../../pages/WordPressProject';
import WordPressDetail from '../../pages/WordPressDetail';

// "WordPress Projects" was renamed to "Pipelines" (§2 unification). Forward old
// deep links to the new space.
function LegacyWpPipelineRedirect() {
    const { id, tab } = useParams();
    const suffix = [id, tab].filter(Boolean).join('/');
    return <Navigate to={`/wordpress/pipelines/${suffix}`} replace />;
}

export function WordPressExtension() {
    return (
        <Routes>
            {/* Tab group — list surfaces share one PageTopbar + WORDPRESS_TABS. */}
            <Route element={<TabGroupLayout tabs={WORDPRESS_TABS} />}>
                <Route index element={<WordPress />} />
                <Route path="plugins/library" element={<WordPressPluginLibrary />} />
                <Route path="pipelines" element={<WordPressProjects />} />
            </Route>
            {/* Full-bleed pipeline detail (own chrome, outside the tab group). */}
            <Route path="pipelines/:id" element={<WordPressProject />} />
            <Route path="pipelines/:id/:tab" element={<WordPressProject />} />
            {/* Legacy "WordPress Projects" URLs → Pipelines (§2). */}
            <Route path="projects" element={<Navigate to="/wordpress/pipelines" replace />} />
            <Route path="projects/:id" element={<LegacyWpPipelineRedirect />} />
            <Route path="projects/:id/:tab" element={<LegacyWpPipelineRedirect />} />
            {/* Full-bleed site detail (own chrome, outside the tab group). The
                :tab/:section depth keeps the Settings sub-nav shareable. */}
            <Route path=":id" element={<WordPressDetail />} />
            <Route path=":id/:tab" element={<WordPressDetail />} />
            <Route path=":id/:tab/:section" element={<WordPressDetail />} />
        </Routes>
    );
}

// No default export on purpose: PluginLoader legacy-auto-renders any plugin
// default export globally — this sub-router mounted that way runs outside its
// route and swallows the current location ("/domains" → :id "domains").
// The route contribution resolves the NAMED export via resolveComponent.
