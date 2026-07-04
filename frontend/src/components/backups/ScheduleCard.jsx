import { useState, useEffect, useMemo } from 'react';
import { SegControl } from '@/components/ds';
import { Switch } from '@/components/ui/switch';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Calendar, Save, Loader2 } from 'lucide-react';

// Card 2 of the backup "Protection" panel: the editable schedule form.
// The saved policy carries a raw cron string; this card translates it into a
// friendly Daily / Weekly / Custom UI and back. Local state mirrors the form
// while a useEffect re-seeds it whenever the parent reloads the policy, so an
// external save stays in sync. Save is disabled until something actually
// changes (dirty tracking) and shows a spinner while the request is in flight.

// Canonical cron strings for the two preset frequencies (02:00 server time).
const DAILY_CRON = '0 2 * * *';
const WEEKLY_CRON = '0 2 * * 0'; // Sunday
// Day-of-week chip labels; index 0 = Sunday … 6 = Saturday (cron dow order).
const DOW_LABELS = ['S', 'M', 'T', 'W', 'T', 'F', 'S'];

const DEFAULT_TIME = '02:00';
const MIN_COUNT = 1; // floor shared by every numeric field (count/days/full-every)

// Two-digit zero-pad for assembling/displaying HH:MM from cron integers.
const pad = (n) => String(n).padStart(2, '0');

// True when a cron token is a plain non-negative integer (e.g. "2", not "*").
const isInt = (token) => /^\d+$/.test(token);

// Coerce a numeric form field (kept as a string) to a number, treating empty
// or sub-floor values as the minimum so we never persist 0 / NaN.
const toNum = (value) => {
    const n = Number(value);
    return Number.isFinite(n) && n >= MIN_COUNT ? n : MIN_COUNT;
};

// Expand a cron day-of-week field into a sorted int array (0..6).
// "*"/"?" -> [] (every day); supports comma lists and "a-b" ranges.
function parseDow(field) {
    if (!field || field === '*' || field === '?') return [];
    const out = new Set();
    for (const part of field.split(',')) {
        const range = part.split('-');
        if (range.length === 2 && isInt(range[0]) && isInt(range[1])) {
            const lo = Number(range[0]);
            const hi = Number(range[1]);
            for (let d = lo; d <= hi; d += 1) {
                if (d >= 0 && d <= 6) out.add(d);
            }
        } else if (isInt(part)) {
            const d = Number(part);
            if (d >= 0 && d <= 6) out.add(d);
        }
    }
    return Array.from(out).sort((a, b) => a - b);
}

// Translate a saved cron string into the form's { frequency, time, days }.
function deriveFromCron(cron) {
    if (cron === DAILY_CRON) return { frequency: 'daily', time: DEFAULT_TIME, days: [] };
    if (cron === WEEKLY_CRON) return { frequency: 'weekly', time: DEFAULT_TIME, days: [0] };

    const fields = (cron || '').trim().split(/\s+/);
    const min = fields[0];
    const hour = fields[1];
    const time = isInt(hour) && isInt(min) ? `${pad(Number(hour))}:${pad(Number(min))}` : DEFAULT_TIME;
    return { frequency: 'custom', time, days: parseDow(fields[4]) };
}

// Assemble a cron string from the form state.
function buildCron(frequency, time, days) {
    if (frequency === 'daily') return DAILY_CRON;
    if (frequency === 'weekly') return WEEKLY_CRON;

    const [hour = '02', min = '00'] = (time || DEFAULT_TIME).split(':');
    const dowField =
        days.length > 0 && days.length < 7
            ? days.slice().sort((a, b) => a - b).join(',')
            : '*';
    return `${Number(min)} ${Number(hour)} * * ${dowField}`;
}

const ScheduleCard = ({ policy, remoteConfigured, onSave, saving }) => {
    // All hooks run unconditionally, before any early return (Rules of Hooks).
    // The form is only shown once a policy exists, so the seed values used when
    // `policy` is null are throwaway and get re-seeded by the effect on load.
    const initial = deriveFromCron(policy?.schedule_cron);

    const [frequency, setFrequency] = useState(initial.frequency);
    const [time, setTime] = useState(initial.time);
    const [days, setDays] = useState(initial.days);
    const [retentionCount, setRetentionCount] = useState(String(policy?.retention_count ?? ''));
    const [retentionDays, setRetentionDays] = useState(String(policy?.retention_days ?? ''));
    const [fullEvery, setFullEvery] = useState(String(policy?.full_every_n_days ?? ''));
    const [compression, setCompression] = useState(policy?.compression ?? 'balanced');
    const [remoteCopy, setRemoteCopy] = useState(!!policy?.remote_copy);

    // Re-seed the form when the saved policy changes (e.g. external reload after
    // a successful save). Keyed on the persisted values so it only fires when
    // the source of truth actually moves, not on every render.
    useEffect(() => {
        if (!policy) return;
        const next = deriveFromCron(policy.schedule_cron);
        setFrequency(next.frequency);
        setTime(next.time);
        setDays(next.days);
        setRetentionCount(String(policy.retention_count));
        setRetentionDays(String(policy.retention_days));
        setFullEvery(String(policy.full_every_n_days));
        setCompression(policy.compression);
        setRemoteCopy(!!policy.remote_copy);
    }, [
        policy,
        policy?.schedule_cron,
        policy?.retention_count,
        policy?.retention_days,
        policy?.full_every_n_days,
        policy?.compression,
        policy?.remote_copy,
    ]);

    const currentCron = useMemo(() => buildCron(frequency, time, days), [frequency, time, days]);

    // Conditional render AFTER all hooks have run.
    if (!policy) {
        return (
            <div className="app-panel schedule-card">
                <div className="app-panel-header">
                    <Calendar size={16} />
                    <span>Schedule</span>
                </div>
                <div className="app-panel-body">
                    <p className="app-panel-hint">Loading backup schedule...</p>
                </div>
            </div>
        );
    }

    const toggleDay = (i) => {
        setDays((prev) =>
            prev.includes(i) ? prev.filter((d) => d !== i) : [...prev, i].sort((a, b) => a - b)
        );
    };

    // The form is dirty when any field diverges from the saved policy. Numeric
    // fields are compared via toNum so an empty box reads as the floor (= what
    // would actually be persisted), avoiding a phantom-dirty Save button.
    const dirty =
        currentCron !== policy.schedule_cron ||
        toNum(retentionCount) !== policy.retention_count ||
        toNum(retentionDays) !== policy.retention_days ||
        toNum(fullEvery) !== policy.full_every_n_days ||
        compression !== policy.compression ||
        remoteCopy !== !!policy.remote_copy;

    const handleSave = () => {
        onSave({
            schedule_cron: currentCron,
            retention_count: toNum(retentionCount),
            retention_days: toNum(retentionDays),
            full_every_n_days: toNum(fullEvery),
            compression,
            remote_copy: remoteCopy,
        });
    };

    return (
        <div className="app-panel schedule-card">
            <div className="app-panel-header">
                <Calendar size={16} />
                <span>Schedule</span>
                <span className="app-panel-header-actions app-panel-hint">
                    Backups run quietly in the background.
                </span>
            </div>
            <div className="app-panel-body">
                <div className="schedule-card__field">
                    <label>Frequency</label>
                    <SegControl
                        options={[
                            { value: 'daily', label: 'Daily' },
                            { value: 'weekly', label: 'Weekly' },
                            { value: 'custom', label: 'Custom' },
                        ]}
                        value={frequency}
                        onChange={setFrequency}
                    />
                </div>

                {frequency === 'custom' && (
                    <>
                        <div className="schedule-card__field">
                            <label>Time</label>
                            <Input type="time" value={time} onChange={(e) => setTime(e.target.value)} />
                        </div>
                        <div className="schedule-card__field">
                            <label>Days</label>
                            <div className="schedule-card__days">
                                {DOW_LABELS.map((d, i) => (
                                    <button
                                        type="button"
                                        key={i}
                                        className={`schedule-card__day ${days.includes(i) ? 'is-active' : ''}`}
                                        onClick={() => toggleDay(i)}
                                    >
                                        {d}
                                    </button>
                                ))}
                            </div>
                        </div>
                    </>
                )}

                <div className="schedule-card__preview">
                    <span className="schedule-card__preview-label">Cron</span>
                    <code>{currentCron}</code>
                </div>

                <div className="schedule-card__field schedule-card__retention">
                    <label>Retention</label>
                    <div className="schedule-card__retention-row">
                        <span>Keep last</span>
                        <Input
                            type="number"
                            min={MIN_COUNT}
                            value={retentionCount}
                            onChange={(e) => setRetentionCount(e.target.value)}
                        />
                        <span>backups</span>
                    </div>
                    <div className="schedule-card__retention-row">
                        <span>Delete older than</span>
                        <Input
                            type="number"
                            min={MIN_COUNT}
                            value={retentionDays}
                            onChange={(e) => setRetentionDays(e.target.value)}
                        />
                        <span>days</span>
                    </div>
                    <p className="app-panel-hint">
                        Both rules apply. A backup is kept only if it is within the last N backups AND
                        within the last N days.
                    </p>
                </div>

                <div className="schedule-card__field">
                    <label>Full backup every</label>
                    <div className="schedule-card__retention-row">
                        <Input
                            type="number"
                            min={MIN_COUNT}
                            value={fullEvery}
                            onChange={(e) => setFullEvery(e.target.value)}
                        />
                        <span>days</span>
                    </div>
                    <p className="app-panel-hint">
                        A full backup is taken every N days; the rest are incremental.
                    </p>
                </div>

                <div className="schedule-card__field">
                    <label>Compression</label>
                    <SegControl
                        options={[
                            { value: 'fast', label: 'Fast' },
                            { value: 'balanced', label: 'Balanced' },
                            { value: 'max', label: 'Max' },
                        ]}
                        value={compression}
                        onChange={setCompression}
                    />
                </div>

                <div className="schedule-card__field schedule-card__remote">
                    <Switch
                        id="remote-copy"
                        checked={remoteCopy}
                        onCheckedChange={setRemoteCopy}
                        disabled={!remoteConfigured}
                    />
                    <label htmlFor="remote-copy">
                        <span>Copy backups to remote storage</span>
                        <span className="app-panel-hint">
                            {remoteConfigured
                                ? 'Uses the provider configured in Backups → Storage.'
                                : 'No remote storage configured. Set one up in Backups → Storage.'}
                        </span>
                    </label>
                </div>

                <div className="schedule-card__actions">
                    <Button
                        variant="primary"
                        size="sm"
                        disabled={!dirty || saving}
                        onClick={handleSave}
                    >
                        {saving ? <Loader2 size={14} className="spin" /> : <Save size={14} />}
                        Save schedule
                    </Button>
                </div>
            </div>
        </div>
    );
};

export default ScheduleCard;
