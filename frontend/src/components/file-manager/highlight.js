// Lightweight line-based syntax tinting for the file preview (prototype
// `hlCode`). Not a parser — just enough token color for config/code files.
// Input is escaped before any markup is inserted, so the produced HTML only
// ever contains our own token spans.

const HASH_COMMENT_EXTS = ['sh', 'bash', 'yml', 'yaml', 'ini', 'env', 'conf', 'caddy', 'toml', 'md', 'cfg'];
const SLASH_COMMENT_EXTS = ['php', 'js', 'jsx', 'ts', 'tsx', 'css', 'scss', 'json'];
const JS_EXTS = ['js', 'jsx', 'ts', 'tsx', 'mjs'];

function esc(s) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

export function highlightLine(line, ext) {
    let h = esc(line);
    if (HASH_COMMENT_EXTS.includes(ext) && /^\s*#/.test(line)) return `<span class="tok-cmt">${h}</span>`;
    if (SLASH_COMMENT_EXTS.includes(ext) && /^\s*(\/\/|\/\*|\*)/.test(line)) return `<span class="tok-cmt">${h}</span>`;

    h = h.replace(/("[^"]*"|'[^']*')/g, '<span class="tok-str">$1</span>');
    h = h.replace(/\b(\d+(?:\.\d+)?[A-Za-z]*)\b/g, '<span class="tok-num">$1</span>');

    if (ext === 'php') {
        h = h.replace(/\b(define|require_once|require|include|getenv|defined|function|return|if|else|new|echo|use|namespace|class|public|private|protected|static)\b/g, '<span class="tok-kw">$1</span>');
        h = h.replace(/(\$\w+)/g, '<span class="tok-var">$1</span>');
        h = h.replace(/(&lt;\?php)/g, '<span class="tok-kw">$1</span>');
    } else if (JS_EXTS.includes(ext)) {
        h = h.replace(/\b(const|let|var|function|return|if|else|new|import|export|from|default|async|await|class|typeof|throw|try|catch)\b/g, '<span class="tok-kw">$1</span>');
    }
    return h;
}

export function fileExt(name) {
    const i = (name || '').lastIndexOf('.');
    return i > 0 ? name.slice(i + 1).toLowerCase() : '';
}
