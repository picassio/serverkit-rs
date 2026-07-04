/**
 * PluginSlot - Renders installed plugin widgets contributed to a named slot.
 *
 * Extensions declare slotted widgets in their manifest:
 *   "contributions": { "widgets": [{ "slot": "dashboard.top",
 *                                     "component": "MyWidget" }] }
 *
 * Mount a slot at any host render point with
 *   <PluginSlot name="dashboard.top" />
 * and every widget whose `slot` matches renders there. Resolution mirrors
 * PluginLoader (build-time module discovery + optional Provider wrap); an
 * unresolved component is skipped rather than crashing the host.
 *
 * The optional `context` object is passed as a prop to each rendered widget so
 * a mount point can hand down page-scoped data (e.g. the current domain or
 * service id). Renders nothing when no widget targets the slot.
 */
import { useContributions, resolveComponent, getPluginModule } from '../plugins/contributions';

const PluginSlot = ({ name, context }) => {
    const { widgets } = useContributions();

    const rendered = (widgets || [])
        .filter((w) => w && w.slot === name)
        .map((w, i) => {
            const Component = resolveComponent(w.plugin, w.component);
            if (!Component) return null; // unresolved → skip, never crash the host
            const mod = getPluginModule(w.plugin);
            const Provider = (mod && mod.Provider) || null;
            const node = <Component key={`${w.plugin}:${w.component}:${i}`} context={context} />;
            return Provider
                ? <Provider key={`provider:${w.plugin}:${i}`}>{node}</Provider>
                : node;
        })
        .filter(Boolean);

    if (rendered.length === 0) return null;
    return <>{rendered}</>;
};

export default PluginSlot;
