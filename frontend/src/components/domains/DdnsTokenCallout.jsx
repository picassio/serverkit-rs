// One-time Dynamic DNS token callout — the update token is only returned once on
// create/regenerate, so we surface it prominently with the ready-to-use update URL.
// Shared by the Domains drawer's per-record "Make dynamic" flow.
import { AlertTriangle } from 'lucide-react';
import CopyButton from '../CopyButton';

export default function DdnsTokenCallout({ host, onDismiss }) {
    // The public, token-authenticated endpoint a router/cron calls on IP change.
    const updateUrl = `${window.location.origin}/api/v1/ddns/update?token=${host.token}`;

    return (
        <div className="ddns-token-callout">
            <div className="ddns-token-callout__head">
                <AlertTriangle size={16} />
                <span>
                    Token for <strong>{host.hostname || host.record_name}</strong> — shown once. Save it now.
                </span>
                {onDismiss && (
                    <button type="button" className="ddns-token-callout__close" onClick={onDismiss} aria-label="Dismiss">
                        &times;
                    </button>
                )}
            </div>

            <div className="ddns-token-callout__row">
                <span className="ddns-token-callout__label">Token</span>
                <code className="ddns-token-callout__value">{host.token}</code>
                <CopyButton value={host.token} label="Copy token" size="sm" variant="outline" />
            </div>

            <div className="ddns-token-callout__row">
                <span className="ddns-token-callout__label">Update URL</span>
                <code className="ddns-token-callout__value">{updateUrl}</code>
                <CopyButton value={updateUrl} label="Copy URL" size="sm" variant="outline" />
            </div>
        </div>
    );
}
