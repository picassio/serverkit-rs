import { useServerkitAI } from '../../contexts/AIContext';

const MODES = [
    { id: 'assistant', label: 'Assistant' },
    { id: 'simple', label: 'Simple' },
];

// Assistant = tools + current-page context; Simple = plain chat, no tools.
const ModeToggle = () => {
    const { mode, setMode } = useServerkitAI();
    return (
        <div className="sk-ai-modes" role="tablist" aria-label="Chat mode">
            {MODES.map((m) => (
                <button
                    key={m.id}
                    type="button"
                    role="tab"
                    aria-selected={mode === m.id}
                    className={`sk-ai-modes__item${mode === m.id ? ' is-active' : ''}`}
                    onClick={() => setMode(m.id)}
                >
                    {m.label}
                </button>
            ))}
        </div>
    );
};

export default ModeToggle;
