import { useRef, useState } from 'react';
import { Send, Square } from 'lucide-react';
import { useServerkitAI } from '../../contexts/AIContext';

const Composer = () => {
    const { send, stop, isStreaming, providerConfigured } = useServerkitAI();
    const [value, setValue] = useState('');
    const textareaRef = useRef(null);

    const submit = () => {
        const text = value.trim();
        if (!text || isStreaming) return;
        send(text);
        setValue('');
        if (textareaRef.current) textareaRef.current.style.height = 'auto';
    };

    const onKeyDown = (e) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            submit();
        }
    };

    const onInput = (e) => {
        setValue(e.target.value);
        const el = e.target;
        el.style.height = 'auto';
        el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
    };

    return (
        <div className="sk-ai-composer">
            <textarea
                ref={textareaRef}
                className="sk-ai-composer__input"
                rows={1}
                placeholder={providerConfigured ? 'Ask about your servers…' : 'Assistant not configured'}
                value={value}
                disabled={!providerConfigured}
                onChange={onInput}
                onKeyDown={onKeyDown}
                aria-label="Message the assistant"
            />
            {isStreaming ? (
                <button type="button" className="sk-ai-composer__btn sk-ai-composer__btn--stop" onClick={stop} aria-label="Stop">
                    <Square size={16} />
                </button>
            ) : (
                <button
                    type="button"
                    className="sk-ai-composer__btn"
                    onClick={submit}
                    disabled={!providerConfigured || !value.trim()}
                    aria-label="Send"
                >
                    <Send size={16} />
                </button>
            )}
        </div>
    );
};

export default Composer;
