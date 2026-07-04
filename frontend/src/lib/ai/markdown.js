// Minimal, dependency-free, XSS-safe markdown renderer for assistant messages.
//
// Strategy: extract fenced code blocks, escape ALL remaining HTML, then apply a
// small allowlist of transforms (inline code, bold, links, paragraphs/line
// breaks). Because everything is escaped before we inject our own tags, raw
// HTML in the model output can never execute. This intentionally supports only
// what a chat assistant typically emits; swap in `marked` + `DOMPurify` later
// if richer markdown is needed.

// Sentinel char that cannot appear in normal text and survives HTML-escaping.
const S = String.fromCharCode(0);

function escapeHtml(s) {
    return s
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;');
}

export function renderMarkdownToHtml(src) {
    if (!src) return '';

    // 1) Pull out fenced code blocks first (contents escaped, never transformed).
    const codeBlocks = [];
    let text = String(src).replace(/```(\w*)\n?([\s\S]*?)```/g, (_m, lang, code) => {
        const i = codeBlocks.length;
        const label = lang ? `<span class="sk-ai-code__lang">${escapeHtml(lang)}</span>` : '';
        codeBlocks.push(
            `<pre class="sk-ai-code">${label}<code>${escapeHtml(code.replace(/\n$/, ''))}</code></pre>`,
        );
        return `${S}CODE${i}${S}`;
    });

    // 2) Escape everything else.
    text = escapeHtml(text);

    // 3) Inline code (escaped content already).
    const inline = [];
    text = text.replace(/`([^`\n]+)`/g, (_m, c) => {
        const i = inline.length;
        inline.push(`<code class="sk-ai-inline-code">${c}</code>`);
        return `${S}INL${i}${S}`;
    });

    // 4) Bold + links.
    text = text.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
    text = text.replace(
        /\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g,
        (_m, label, url) => {
            // The surrounding text is already HTML-escaped, but `"` is not, and a
            // crafted URL could otherwise break out of the href attribute. Encode
            // quotes so the value stays contained.
            const safeUrl = url.replace(/"/g, '%22').replace(/'/g, '%27');
            return `<a href="${safeUrl}" target="_blank" rel="noopener noreferrer">${label}</a>`;
        },
    );

    // 5) Paragraphs + line breaks.
    text = text
        .split(/\n{2,}/)
        .map((para) => {
            const block = para.trim();
            if (!block) return '';
            if (new RegExp(`^${S}CODE\\d+${S}$`).test(block)) return block; // standalone code block
            return `<p>${block.replace(/\n/g, '<br>')}</p>`;
        })
        .join('');

    // 6) Restore placeholders.
    text = text.replace(new RegExp(`${S}INL(\\d+)${S}`, 'g'), (_m, i) => inline[+i] || '');
    text = text.replace(new RegExp(`${S}CODE(\\d+)${S}`, 'g'), (_m, i) => codeBlocks[+i] || '');
    return text;
}
