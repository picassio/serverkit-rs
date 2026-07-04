// Allowlist sanitizer for the inline SVG icon strings rendered in the sidebar.
//
// Core sidebar icons are trusted constants, but plugin-contributed nav items
// flow through the same render path and could supply arbitrary SVG (script,
// event handlers, <foreignObject>, external refs). The icons are a small, fixed
// shape grammar, so we parse the string with the real XML parser and rebuild it
// keeping ONLY known-safe elements and attributes — anything else (incl. every
// on*/event handler) is dropped. Results are cached since icon strings are
// stable across renders.

const ALLOWED_TAGS = new Set([
    'path', 'rect', 'circle', 'ellipse', 'line', 'polyline', 'polygon',
    'g', 'defs', 'lineargradient', 'radialgradient', 'stop', 'clippath',
    'title', 'desc', 'tspan', 'text', 'svg',
]);

const ALLOWED_ATTRS = new Set([
    'd', 'x', 'y', 'width', 'height', 'cx', 'cy', 'r', 'rx', 'ry',
    'x1', 'y1', 'x2', 'y2', 'points', 'transform', 'viewbox',
    'preserveaspectratio', 'fill', 'stroke', 'stroke-width', 'stroke-linecap',
    'stroke-linejoin', 'stroke-dasharray', 'stroke-dashoffset', 'opacity',
    'fill-opacity', 'stroke-opacity', 'fill-rule', 'clip-rule', 'clip-path',
    'offset', 'stop-color', 'stop-opacity', 'gradientunits',
    'gradienttransform', 'id', 'class', 'text-anchor', 'font-size', 'dx', 'dy',
]);

const cache = new Map();

export function sanitizeSvgInner(svg) {
    if (!svg || typeof svg !== 'string') return '';
    if (cache.has(svg)) return cache.get(svg);
    if (typeof DOMParser === 'undefined') return '';

    let result = '';
    try {
        const doc = new DOMParser().parseFromString(
            `<svg xmlns="http://www.w3.org/2000/svg">${svg}</svg>`,
            'image/svg+xml'
        );
        const root = doc.documentElement;
        if (root && !root.getElementsByTagName('parsererror').length) {
            scrub(root);
            result = root.innerHTML;
        }
    } catch {
        result = '';
    }

    cache.set(svg, result);
    return result;
}

function scrub(node) {
    for (const child of Array.from(node.children)) {
        const tag = child.tagName.toLowerCase();
        if (!ALLOWED_TAGS.has(tag)) {
            child.remove();
            continue;
        }
        for (const attr of Array.from(child.attributes)) {
            const name = attr.name.toLowerCase();
            if (name.startsWith('on') || !ALLOWED_ATTRS.has(name)) {
                child.removeAttribute(attr.name);
            }
        }
        scrub(child);
    }
}

export default sanitizeSvgInner;
