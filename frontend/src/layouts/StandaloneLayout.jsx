/**
 * Standalone layout — no sidebar, no chrome, just the route's outlet.
 *
 * Used for the built-in `bare` layout id: plugin pages that want the
 * whole viewport with no host UI around them (think: a remote-desktop
 * console, a fullscreen editor, a kiosk view). Authentication is still
 * enforced by the parent PrivateRoute in App.jsx.
 */
import { Outlet } from 'react-router-dom';

const StandaloneLayout = () => <Outlet />;

export default StandaloneLayout;
