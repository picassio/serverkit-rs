import { useMemo } from 'react';
import { renderMarkdownToHtml } from '../../lib/ai/markdown';

// Renders assistant text. The renderer escapes all input before injecting its
// own allowlisted tags, so dangerouslySetInnerHTML is safe here.
const Markdown = ({ text }) => {
    const html = useMemo(() => renderMarkdownToHtml(text || ''), [text]);
    return <div className="sk-ai-markdown" dangerouslySetInnerHTML={{ __html: html }} />;
};

export default Markdown;
