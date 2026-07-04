#!/usr/bin/env node
/**
 * Sync builtin-extension frontends into the pre-bundled plugin tree.
 *
 * Decision D5 / task #3 of docs/plans/12_EXTENSIONS_PLATFORM_PLAN.md:
 *
 *   `builtin-extensions/<slug>/frontend/` is the SINGLE SOURCE OF TRUTH for a
 *   builtin extension's UI. `frontend/src/plugins/<slug>/` is a *build artifact*
 *   of it — checked in only so Vite's build-time `import.meta.glob` can compile
 *   the extension into every shipped bundle (the production frontend-delivery
 *   constraint documented in the plan §1.3).
 *
 * Historically the two copies were hand-maintained and silently drifted. This
 * script makes the relationship mechanical:
 *
 *   - `node scripts/sync-builtin-frontends.mjs`          → copy source → artifact
 *   - `node scripts/sync-builtin-frontends.mjs --check`  → fail (exit 1) on drift
 *
 * Only builtin extensions that actually ship a `frontend/` dir are managed.
 * Other entries under frontend/src/plugins/ (e.g. serverkit-gui, whose source
 * lives in a sibling repo, and the shared sdk/ + loader files) are left alone.
 */
import { promises as fs } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const BUILTIN_DIR = path.join(ROOT, 'builtin-extensions');
const PLUGINS_DIR = path.join(ROOT, 'frontend', 'src', 'plugins');

const CHECK = process.argv.includes('--check');

async function exists(p) {
    try { await fs.access(p); return true; } catch { return false; }
}

async function readJson(p) {
    return JSON.parse(await fs.readFile(p, 'utf8'));
}

// Canonical on-disk form of a manifest: 2-space indent + trailing newline.
// Normalizing here means the source manifest's own formatting is irrelevant —
// the check compares semantic content, not whitespace.
function canonicalManifest(manifest) {
    return JSON.stringify(manifest, null, 2) + '\n';
}

// Recursively collect relative file paths under `dir` (posix separators).
async function walk(dir, base = dir) {
    const out = [];
    let entries;
    try {
        entries = await fs.readdir(dir, { withFileTypes: true });
    } catch {
        return out;
    }
    for (const e of entries.sort((a, b) => a.name.localeCompare(b.name))) {
        const full = path.join(dir, e.name);
        if (e.isDirectory()) {
            out.push(...await walk(full, base));
        } else if (e.isFile()) {
            out.push(path.relative(base, full).split(path.sep).join('/'));
        }
    }
    return out;
}

// Build the expected file map { relPath: Buffer } for a managed slug.
async function expectedFiles(builtinSlugDir, manifest) {
    const map = new Map();
    const frontendSrc = path.join(builtinSlugDir, 'frontend');
    for (const rel of await walk(frontendSrc)) {
        map.set(rel, await fs.readFile(path.join(frontendSrc, rel)));
    }
    // The pre-bundled copy also carries the manifest so contributions.js can
    // read it at build time (import.meta.glob of plugin.json).
    map.set('plugin.json', Buffer.from(canonicalManifest(manifest), 'utf8'));
    return map;
}

async function main() {
    if (!(await exists(BUILTIN_DIR))) {
        console.error(`No builtin-extensions dir at ${BUILTIN_DIR}`);
        process.exit(CHECK ? 1 : 0);
    }

    const folders = (await fs.readdir(BUILTIN_DIR, { withFileTypes: true }))
        .filter((e) => e.isDirectory())
        .map((e) => e.name)
        .sort();

    const drift = [];
    let managed = 0;

    for (const folder of folders) {
        const slugDir = path.join(BUILTIN_DIR, folder);
        const manifestPath = path.join(slugDir, 'plugin.json');
        const frontendSrc = path.join(slugDir, 'frontend');

        // Only builtins that ship a frontend are managed here.
        if (!(await exists(manifestPath)) || !(await exists(frontendSrc))) continue;

        const manifest = await readJson(manifestPath);
        const slug = manifest.name || folder;
        const target = path.join(PLUGINS_DIR, slug);
        managed += 1;

        const expected = await expectedFiles(slugDir, manifest);

        if (CHECK) {
            const actualRel = new Set(await walk(target));
            for (const [rel, buf] of expected) {
                const cur = path.join(target, rel);
                if (!(await exists(cur))) {
                    drift.push(`missing  ${slug}/${rel}`);
                    continue;
                }
                const got = await fs.readFile(cur);
                if (!got.equals(buf)) drift.push(`differs  ${slug}/${rel}`);
                actualRel.delete(rel);
            }
            for (const rel of actualRel) drift.push(`extra    ${slug}/${rel}`);
        } else {
            // Write mode: mirror source → artifact, pruning stale files.
            const actualRel = new Set(await walk(target));
            for (const [rel, buf] of expected) {
                const cur = path.join(target, rel);
                await fs.mkdir(path.dirname(cur), { recursive: true });
                await fs.writeFile(cur, buf);
                actualRel.delete(rel);
            }
            for (const rel of actualRel) {
                await fs.rm(path.join(target, rel));
            }
            console.log(`synced   ${slug} (${expected.size} files)`);
        }
    }

    if (CHECK) {
        if (drift.length) {
            console.error('Builtin-extension frontend drift detected:\n');
            for (const d of drift) console.error(`  ${d}`);
            console.error(
                '\nThe pre-bundled copy under frontend/src/plugins/ is out of sync '
                + 'with its source under builtin-extensions/<slug>/frontend/.\n'
                + 'Run:  node scripts/sync-builtin-frontends.mjs   and commit the result.'
            );
            process.exit(1);
        }
        console.log(`Builtin-extension frontends in sync (${managed} managed).`);
    }
}

main().catch((e) => {
    console.error(e);
    process.exit(1);
});
