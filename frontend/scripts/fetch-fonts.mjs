#!/usr/bin/env node
// ============================================================
// fetch-fonts.mjs — vendor IBM Plex woff2 into frontend/public/fonts/
// ============================================================
// ServerKit self-hosts its fonts for privacy (no Google Fonts CDN — that would
// leak every visitor's IP + usage to a third party). This script downloads the
// OFL-licensed IBM Plex faces once, from the @fontsource jsDelivr mirror, into
// the repo's public/fonts/ folder so they're served from our own origin.
//
//   node frontend/scripts/fetch-fonts.mjs
//
// Re-run only when changing weights. Commit the resulting .woff2 files.
// Requires Node 18+ (global fetch). No network at build/runtime — only here.
// ============================================================

import { mkdir, writeFile, access } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const OUT = join(__dirname, '..', 'public', 'fonts');
const BASE = 'https://cdn.jsdelivr.net/npm';

// local filename  ->  @fontsource source path
const FONTS = {
    'ibm-plex-sans-400.woff2': '@fontsource/ibm-plex-sans/files/ibm-plex-sans-latin-400-normal.woff2',
    'ibm-plex-sans-500.woff2': '@fontsource/ibm-plex-sans/files/ibm-plex-sans-latin-500-normal.woff2',
    'ibm-plex-sans-600.woff2': '@fontsource/ibm-plex-sans/files/ibm-plex-sans-latin-600-normal.woff2',
    'ibm-plex-sans-700.woff2': '@fontsource/ibm-plex-sans/files/ibm-plex-sans-latin-700-normal.woff2',
    'ibm-plex-mono-400.woff2': '@fontsource/ibm-plex-mono/files/ibm-plex-mono-latin-400-normal.woff2',
    'ibm-plex-mono-500.woff2': '@fontsource/ibm-plex-mono/files/ibm-plex-mono-latin-500-normal.woff2',
    'ibm-plex-mono-600.woff2': '@fontsource/ibm-plex-mono/files/ibm-plex-mono-latin-600-normal.woff2',
};

async function main() {
    await mkdir(OUT, { recursive: true });
    let ok = 0;
    for (const [name, path] of Object.entries(FONTS)) {
        const dest = join(OUT, name);
        try {
            await access(dest);
            console.log(`✓ ${name} (already present)`);
            ok++;
            continue;
        } catch { /* not present — download */ }

        const url = `${BASE}/${path}`;
        try {
            const res = await fetch(url);
            if (!res.ok) throw new Error(`HTTP ${res.status}`);
            const buf = Buffer.from(await res.arrayBuffer());
            await writeFile(dest, buf);
            console.log(`↓ ${name}  (${(buf.length / 1024).toFixed(1)} KB)`);
            ok++;
        } catch (err) {
            console.error(`✗ ${name} — ${err.message}\n   ${url}`);
        }
    }
    console.log(`\n${ok}/${Object.keys(FONTS).length} fonts in ${OUT}`);
    if (ok < Object.keys(FONTS).length) process.exitCode = 1;
}

main();
