import { forwardRef, useMemo, useRef } from 'react';

// SQL console editor with a line-number gutter + keyword tinting (the demo's
// .sql-editor). Classic overlay technique: a transparent <textarea> captures
// input over a highlighted <pre> kept scroll-synced — both must share the
// exact same font metrics (see .dbx-sqled in _databases.scss).

const SQL_KEYWORDS = /\b(select|from|where|order\s+by|group\s+by|having|limit|offset|insert\s+into|insert|update|delete|join|left|right|inner|outer|cross|on|as|and|or|not|null|is|in|like|between|exists|union|all|distinct|create|table|database|drop|alter|index|primary|key|foreign|references|values|set|into|desc|asc|show|describe|explain|count|sum|avg|min|max|case|when|then|else|end|begin|commit|rollback|truncate)\b/gi;

function esc(s) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function hlSql(line) {
    let h = esc(line);
    if (/^\s*(--|#)/.test(line)) return `<span class="tok-cmt">${h}</span>`;
    h = h.replace(/("[^"]*"|'[^']*'|`[^`]*`)/g, '<span class="tok-str">$1</span>');
    h = h.replace(/\b(\d+(?:\.\d+)?)\b/g, '<span class="tok-num">$1</span>');
    h = h.replace(SQL_KEYWORDS, '<span class="tok-kw">$&</span>');
    return h;
}

const SqlEditor = forwardRef(function SqlEditor({ value, onChange, onKeyDown, placeholder, ariaLabel }, ref) {
    const preRef = useRef(null);
    const gutterRef = useRef(null);

    const lines = useMemo(() => (value ?? '').split('\n'), [value]);
    // trailing blank line keeps the pre's scrollHeight in step with the textarea
    const html = useMemo(() => lines.map(hlSql).join('\n') + '\n', [lines]);

    const syncScroll = (e) => {
        if (preRef.current) {
            preRef.current.scrollTop = e.target.scrollTop;
            preRef.current.scrollLeft = e.target.scrollLeft;
        }
        if (gutterRef.current) gutterRef.current.scrollTop = e.target.scrollTop;
    };

    return (
        <div className="dbx-sqled">
            <div className="dbx-sqled__gutter" ref={gutterRef} aria-hidden="true">
                {lines.map((_, i) => <div key={i}>{i + 1}</div>)}
            </div>
            <div className="dbx-sqled__stack">
                <pre
                    className="dbx-sqled__hl"
                    ref={preRef}
                    aria-hidden="true"
                    dangerouslySetInnerHTML={{ __html: html }}
                />
                <textarea
                    ref={ref}
                    className="dbx-sqled__input"
                    value={value}
                    onChange={onChange}
                    onKeyDown={onKeyDown}
                    onScroll={syncScroll}
                    placeholder={placeholder}
                    spellCheck={false}
                    autoCapitalize="off"
                    autoCorrect="off"
                    wrap="off"
                    aria-label={ariaLabel}
                />
            </div>
        </div>
    );
});

export default SqlEditor;
