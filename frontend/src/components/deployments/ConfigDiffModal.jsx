import { useState, useEffect } from 'react';
import { RotateCcw, Info } from 'lucide-react';
import api from '../../services/api';
import Modal from '@/components/Modal';
import { Button } from '@/components/ui/button';
import { useToast } from '../../contexts/ToastContext';

// Modal showing a structured config diff between a snapshot and another
// (default: the previous snapshot). Env vars are compared by KEY only — values
// are never shown, so secrets can't leak. Sections: env (added/removed/changed
// keys), domains, image/tag, build method, volumes. A Restore button re-applies
// the snapshot's config and triggers a redeploy.

function DiffList({ title, added = [], removed = [], changed = [] }) {
    const hasAny = added.length || removed.length || changed.length;
    if (!hasAny) {
        return (
            <div className="config-diff__section">
                <h4 className="config-diff__section-title">{title}</h4>
                <p className="config-diff__none">No changes</p>
            </div>
        );
    }
    return (
        <div className="config-diff__section">
            <h4 className="config-diff__section-title">{title}</h4>
            <ul className="config-diff__lines">
                {added.map((k) => (
                    <li key={`a-${k}`} className="config-diff__line config-diff__line--add">
                        <span className="config-diff__sign">+</span> {k}
                    </li>
                ))}
                {removed.map((k) => (
                    <li key={`r-${k}`} className="config-diff__line config-diff__line--remove">
                        <span className="config-diff__sign">−</span> {k}
                    </li>
                ))}
                {changed.map((k) => (
                    <li key={`c-${k}`} className="config-diff__line config-diff__line--change">
                        <span className="config-diff__sign">~</span> {k}
                    </li>
                ))}
            </ul>
        </div>
    );
}

function ScalarDiff({ title, oldVal, newVal, changed }) {
    return (
        <div className="config-diff__section">
            <h4 className="config-diff__section-title">{title}</h4>
            {changed ? (
                <div className="config-diff__scalar">
                    <span className="config-diff__old">{oldVal || '—'}</span>
                    <span className="config-diff__arrow">→</span>
                    <span className="config-diff__new">{newVal || '—'}</span>
                </div>
            ) : (
                <p className="config-diff__none">No change ({newVal || '—'})</p>
            )}
        </div>
    );
}

const ConfigDiffModal = ({ appId, snapId, against = 'previous', onClose, onRestored }) => {
    const toast = useToast();
    const [diff, setDiff] = useState(null);
    const [meta, setMeta] = useState(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);
    const [restoring, setRestoring] = useState(false);
    const [confirmRestore, setConfirmRestore] = useState(false);

    useEffect(() => {
        let cancelled = false;
        (async () => {
            try {
                setLoading(true);
                const res = await api.getSnapshotDiff(appId, snapId, against);
                if (cancelled) return;
                setDiff(res.diff);
                setMeta({
                    summary: res.summary,
                    hasChanges: res.has_changes,
                    againstId: res.against_id,
                });
                setError(null);
            } catch (err) {
                if (!cancelled) setError(err.message);
            } finally {
                if (!cancelled) setLoading(false);
            }
        })();
        return () => {
            cancelled = true;
        };
    }, [appId, snapId, against]);

    async function handleRestore() {
        setRestoring(true);
        try {
            const res = await api.restoreSnapshot(appId, snapId);
            if (res.success) {
                toast.success('Configuration restored. Redeploy triggered.');
                if (onRestored) onRestored(res);
            } else {
                toast.error(res.error || 'Restore failed');
            }
        } catch (err) {
            toast.error(err.message);
        } finally {
            setRestoring(false);
            setConfirmRestore(false);
        }
    }

    return (
        <Modal open onClose={onClose} title="Configuration diff" size="xl" className="config-diff">
            <div className="config-diff__body">
                    {loading && <p className="config-diff__loading">Loading diff…</p>}
                    {error && <div className="alert alert-danger">{error}</div>}

                    {!loading && !error && diff && (
                        <>
                            {meta && (
                                <div className="config-diff__summary-banner" role="status">
                                    <Info size={16} className="config-diff__summary-icon" />
                                    <div className="config-diff__summary-text">
                                        <span className="config-diff__summary-label">
                                            In plain language
                                        </span>
                                        <p className="config-diff__summary">
                                            {meta.hasChanges && meta.summary
                                                ? meta.summary
                                                : 'No configuration changes vs the compared checkpoint.'}
                                        </p>
                                    </div>
                                </div>
                            )}

                            <DiffList
                                title="Environment variables"
                                added={diff.env?.added}
                                removed={diff.env?.removed}
                                changed={diff.env?.changed}
                            />
                            <DiffList
                                title="Domains"
                                added={diff.domains?.added}
                                removed={diff.domains?.removed}
                            />
                            <DiffList
                                title="Volumes"
                                added={diff.volumes?.added}
                                removed={diff.volumes?.removed}
                            />
                            <ScalarDiff
                                title="Image / tag"
                                oldVal={diff.image?.old}
                                newVal={diff.image?.new}
                                changed={diff.image?.changed}
                            />
                            <ScalarDiff
                                title="Build method"
                                oldVal={diff.build_method?.old}
                                newVal={diff.build_method?.new}
                                changed={diff.build_method?.changed}
                            />
                        </>
                    )}
                </div>

                <div className="modal-actions">
                    {confirmRestore ? (
                        <>
                            <span className="config-diff__confirm-text">
                                {meta && meta.hasChanges && meta.summary
                                    ? `Restore this configuration and redeploy? ${meta.summary}.`
                                    : 'Restore this configuration and redeploy?'}
                            </span>
                            <Button
                                variant="outline"
                                onClick={() => setConfirmRestore(false)}
                                disabled={restoring}
                            >
                                Cancel
                            </Button>
                            <Button onClick={handleRestore} disabled={restoring}>
                                {restoring ? 'Restoring…' : 'Confirm restore'}
                            </Button>
                        </>
                    ) : (
                        <>
                            <Button variant="outline" onClick={onClose}>
                                Close
                            </Button>
                            <Button onClick={() => setConfirmRestore(true)} disabled={loading || !!error}>
                                <RotateCcw size={14} /> Restore this config
                            </Button>
                        </>
                    )}
                </div>
        </Modal>
    );
};

export default ConfigDiffModal;
