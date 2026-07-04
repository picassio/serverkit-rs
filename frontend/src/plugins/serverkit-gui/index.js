// Plugin entry — discovered by ServerKit's PluginLoader.
// Exports the default widget component used as a global drop-in.
//
// We currently render a floating launcher that pops the Agent GUI viewer in a
// modal scoped to whichever server detail page is open. Embedding inside the
// server detail page as a real tab requires a tiny core change to ServerKit
// (extension points in PluginLoader); see README "Roadmap".

import ServerGuiLauncher from './components/ServerGuiLauncher.jsx';
import './styles/server-gui.css';

export default ServerGuiLauncher;

// Also export the inline tab component so a future PluginLoader extension
// point can mount it directly inside ServerDetail.
export { default as ServerGuiTab } from './components/ServerGui.jsx';
