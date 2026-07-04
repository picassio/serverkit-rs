// Incremental Server-Sent-Events parser for the AI chat stream.
//
// Frames are separated by a blank line. Each frame has an optional `event:`
// line and one or more `data:` lines (JSON). Lines starting with `:` are
// comments (our keepalive heartbeat) and are ignored. Returns the parsed
// events plus any trailing partial frame to carry into the next read.

export function parseSSEChunk(buffer) {
    const events = [];
    const sep = buffer.lastIndexOf('\n\n');
    if (sep === -1) {
        return { events, rest: buffer };
    }
    const complete = buffer.slice(0, sep);
    const rest = buffer.slice(sep + 2);

    for (const block of complete.split('\n\n')) {
        if (!block.trim()) continue;
        let eventName = 'message';
        const dataLines = [];
        for (const line of block.split('\n')) {
            if (line.startsWith(':')) continue; // comment / keepalive
            if (line.startsWith('event:')) {
                eventName = line.slice(6).trim();
            } else if (line.startsWith('data:')) {
                dataLines.push(line.slice(5).replace(/^ /, ''));
            }
        }
        if (!dataLines.length && eventName === 'message') continue;
        const raw = dataLines.join('\n');
        let data = {};
        try {
            data = raw ? JSON.parse(raw) : {};
        } catch {
            data = { raw };
        }
        events.push({ event: eventName, data });
    }
    return { events, rest };
}
