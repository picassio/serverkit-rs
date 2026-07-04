/**
 * ServerKit Plugin SDK
 *
 * Re-exports the core building blocks plugins need so they don't have to
 * reach into deep host paths. Plugins should import from this entry only:
 *
 *     import { api, useToast, Button } from '../../sdk';
 *
 * If we ever rename or restructure the host, plugins keep working as
 * long as the SDK surface stays stable.
 *
 * What a plugin's frontend can ship:
 *
 *   index.js / index.jsx           — main module, exports named React
 *                                    components matched by `component`
 *                                    references in the manifest's
 *                                    `contributions` block.
 *   plugin.json                    — copy of the manifest, written by the
 *                                    backend installer; included so Vite
 *                                    can read static metadata at build
 *                                    time if a plugin wants to.
 *   styles/*.scss|css|less         — auto-discovered and listed in
 *                                    plugins-manifest.json.
 *
 * The host fetches /api/v1/plugins/contributions at runtime; each
 * contribution's `component` field is matched against the named exports
 * of this plugin's index module. A contribution with no matching export
 * is skipped (and logged in dev).
 */

export { api, default as defaultApi } from '../../services/api';

// Common UI primitives plugins are likely to want. Re-exports kept thin
// on purpose — plugins can still reach for niche components directly,
// but the everyday surface lives here.
export { useToast } from '../../contexts/ToastContext';
export { useAuth } from '../../contexts/AuthContext';
export { useTheme } from '../../contexts/ThemeContext';

// AI assistant — plugins consume the core assistant rather than building their
// own chat UI. `useServerkitAI()` exposes:
//   open() / close() / toggle() / isOpen
//   ask(prompt, { context?, mode?: 'assistant'|'simple', open?=true })
//   mode / setMode(mode)
//   registerContextProvider(routePattern, () => contextObj) -> unregister
//   registerToolRenderer(toolName, Component) -> unregister
//   isStreaming / providerConfigured
//
// Plugins can also declare contributions.ai = { suggested_prompts: [{route,label,prompt}],
// tool_renderers: [{tool, component}] } in plugin.json.
export { useServerkitAI, AIContext } from '../../contexts/AIContext';

// Routing helpers — plugin pages need these to navigate within the SPA.
export {
    Link,
    NavLink,
    useNavigate,
    useLocation,
    useParams,
    Outlet,
} from 'react-router-dom';
