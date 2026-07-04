import { useEffect, useRef } from 'react';
import { ShieldAlert } from 'lucide-react';
import { useServerkitAI } from '../../contexts/AIContext';

const formatParams = (params) => {
    if (!params || !Object.keys(params).length) return null;
    try { return JSON.stringify(params, null, 2); } catch { return String(params); }
};

// Inline approve/deny card for a guarded write tool. The model only ever
// proposes; the action runs server-side only after the user approves.
const ConfirmActionCard = () => {
    const { pendingConfirm, confirmAction } = useServerkitAI();
    const denyRef = useRef(null);

    useEffect(() => {
        if (pendingConfirm) denyRef.current?.focus();
    }, [pendingConfirm]);

    if (!pendingConfirm) return null;
    const params = formatParams(pendingConfirm.params);

    return (
        <div className="sk-ai-confirm" role="group" aria-label="Confirm action">
            <div className="sk-ai-confirm__head">
                <ShieldAlert size={16} />
                <span>Confirm action</span>
            </div>
            <p className="sk-ai-confirm__summary">{pendingConfirm.summary}</p>
            {params ? (
                <pre className="sk-ai-code sk-ai-confirm__params"><code>{params}</code></pre>
            ) : null}
            <div className="sk-ai-confirm__actions">
                <button
                    ref={denyRef}
                    type="button"
                    className="sk-ai-btn sk-ai-btn--ghost"
                    onClick={() => confirmAction('deny')}
                >
                    Deny
                </button>
                <button
                    type="button"
                    className="sk-ai-btn sk-ai-btn--danger"
                    onClick={() => confirmAction('approve')}
                >
                    Approve &amp; run
                </button>
            </div>
        </div>
    );
};

export default ConfirmActionCard;
