// FTP Server, contributed through the extension system. The page component and
// its /api/v1/ftp backend stay in core for now (two-speed extraction, D2); this
// extension owns the /ftp tab in the core Files group via a tab-group
// contribution (#43) plus the routes/palette entries in its manifest.
//
// After sync this file lives at frontend/src/plugins/serverkit-ftp/ so the
// relative import resolves against the host's pages directory.
import FTPServer from '../../pages/FTPServer';

export function FtpServerPage() {
    return <FTPServer />;
}
