import { Navigate } from 'react-router-dom';
import useModules from '../hooks/useModules';

// Route guard for the optional feature modules (Email, WordPress). When the
// named module has been disabled by an admin, its routes redirect to the
// dashboard instead of rendering a dead page. While module state is still
// loading it is treated as enabled, so a valid deep link never flashes a
// redirect before state arrives.
const ModuleRoute = ({ name, children }) => {
    const { isEnabled } = useModules();

    if (!isEnabled(name)) {
        return <Navigate to="/" replace />;
    }

    return children;
};

export default ModuleRoute;
