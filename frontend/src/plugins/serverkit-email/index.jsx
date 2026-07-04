// Email Server, contributed through the extension system. The full mail-server
// backend (Postfix/Dovecot/DKIM/SpamAssassin/Roundcube orchestration + the
// /api/v1/email blueprint) lives in this extension's backend/; the page
// component stays in the host pages/ dir and is re-exported here (same pattern
// as serverkit-git/gpu/workflows) so its many relative imports keep resolving.
//
// After sync this file lives at frontend/src/plugins/serverkit-email/ so the
// relative import resolves against the host's pages directory.
import Email from '../../pages/Email';

export function EmailPage() {
    return <Email />;
}
