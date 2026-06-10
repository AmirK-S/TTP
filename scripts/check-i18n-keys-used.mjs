#!/usr/bin/env node
// TTP - Talk To Paste
// Build-time validator: every `t('key.path')` call in TSX/TS source must
// refer to a real leaf in `src/i18n/locales/en.json`.
//
// Catches the class of bug where a copy change in the React layer drifts
// from the locale file — the UI renders the literal key string ("error.foo")
// instead of the translated message and the user sees a placeholder.
//
// Limits (intentional, to keep the scanner simple):
//   - Only literal string arguments are checked: `t('error.no_speech')`.
//   - Template literals and computed keys (`t(`error.${cat}`)`) are skipped
//     since their value isn't known at scan time. False negatives there are
//     accepted; the parity check + manual review cover them.
//   - Single-quote and double-quote literals are accepted; backticks are
//     skipped (per above).

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(here, '..');
const srcDir = path.join(root, 'src');
const enPath = path.join(srcDir, 'i18n', 'locales', 'en.json');

/** Recursively collect every dot-separated leaf path in a translation object. */
function collectLeaves(obj, prefix = '') {
  if (obj === null || typeof obj !== 'object' || Array.isArray(obj)) {
    return new Set([prefix]);
  }
  const out = new Set();
  for (const key of Object.keys(obj)) {
    const next = prefix ? `${prefix}.${key}` : key;
    for (const leaf of collectLeaves(obj[key], next)) {
      out.add(leaf);
    }
  }
  return out;
}

/** Walk a directory and return every .ts/.tsx path. */
function listTsFiles(dir) {
  const out = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.name.startsWith('.') || entry.name === 'node_modules') continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...listTsFiles(full));
    } else if (
      entry.isFile() &&
      (entry.name.endsWith('.ts') || entry.name.endsWith('.tsx')) &&
      !entry.name.endsWith('.test.ts') &&
      !entry.name.endsWith('.test.tsx') &&
      !entry.name.endsWith('.spec.ts') &&
      !entry.name.endsWith('.spec.tsx')
    ) {
      out.push(full);
    }
  }
  return out;
}

// Match `t('key.path')` or `t("key.path")` with optional whitespace and
// optional second argument. Negative lookbehind for `\b` so we don't catch
// methods named `.t(...)` (e.g. `something.t('foo')` if it ever existed).
// Also include `i18n.t(...)` since some windows use that explicit form.
const T_CALL_RE = /(?:\bi18n\.t|\bt)\s*\(\s*(['"])([^'"]+)\1/g;

const enJson = JSON.parse(fs.readFileSync(enPath, 'utf8'));
const validKeys = collectLeaves(enJson);

const files = listTsFiles(srcDir);
const missing = new Map(); // key -> list of "relPath:line" references

for (const file of files) {
  const content = fs.readFileSync(file, 'utf8');
  const lines = content.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    // Skip imports + commented lines — they're not real call sites.
    if (line.trimStart().startsWith('//')) continue;
    if (line.trimStart().startsWith('*')) continue;
    let m;
    T_CALL_RE.lastIndex = 0;
    while ((m = T_CALL_RE.exec(line)) !== null) {
      const key = m[2];
      // Skip keys with template-literal placeholders or whitespace — these
      // were caught as literals only because of pathological regex flow.
      if (!key || key.includes('${') || key.includes(' ')) continue;
      // Skip non-namespaced strings (someone passing a literal as a label,
      // not a translation key). We only validate paths with a dot.
      if (!key.includes('.')) continue;
      if (!validKeys.has(key)) {
        const rel = path.relative(root, file);
        const refs = missing.get(key) ?? [];
        refs.push(`${rel}:${i + 1}`);
        missing.set(key, refs);
      }
    }
  }
}

if (missing.size === 0) {
  console.log(`[i18n-keys] OK — every t() call site resolves against ${validKeys.size} keys in en.json`);
  process.exit(0);
}

console.error(`[i18n-keys] Missing keys referenced from source (${missing.size}):`);
for (const [key, refs] of [...missing.entries()].sort()) {
  console.error(`  - ${key}`);
  for (const ref of refs) {
    console.error(`      ${ref}`);
  }
}
process.exit(1);
