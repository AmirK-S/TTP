#!/usr/bin/env node
// TTP - Talk To Paste
// Parity check: every key in en.json must exist in fr.json and vice versa.
// Catches missing translations at build time before they ship as "translation
// key" placeholders in the UI.
//
// Run from project root: `node scripts/check-i18n-parity.mjs`.
// Wired into `npm run build` so CI fails on mismatches.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const localesDir = path.join(here, '..', 'src', 'i18n', 'locales');

/** Recursively collect dot-separated leaf paths. Leaves are non-object values. */
function collectPaths(obj, prefix = '') {
  if (obj === null || typeof obj !== 'object' || Array.isArray(obj)) {
    return [prefix];
  }
  const out = [];
  for (const key of Object.keys(obj)) {
    const next = prefix ? `${prefix}.${key}` : key;
    out.push(...collectPaths(obj[key], next));
  }
  return out;
}

function loadLocale(name) {
  const file = path.join(localesDir, `${name}.json`);
  const raw = fs.readFileSync(file, 'utf8');
  try {
    return JSON.parse(raw);
  } catch (e) {
    console.error(`[i18n-parity] ${name}.json is not valid JSON: ${e.message}`);
    process.exit(1);
  }
}

const en = loadLocale('en');
const fr = loadLocale('fr');

const enSet = new Set(collectPaths(en));
const frSet = new Set(collectPaths(fr));

const missingInFr = [...enSet].filter((k) => !frSet.has(k)).sort();
const missingInEn = [...frSet].filter((k) => !enSet.has(k)).sort();

if (missingInFr.length === 0 && missingInEn.length === 0) {
  console.log(`[i18n-parity] OK — ${enSet.size} keys match across en and fr`);
  process.exit(0);
}

if (missingInFr.length > 0) {
  console.error(`[i18n-parity] Missing in fr.json (${missingInFr.length}):`);
  for (const k of missingInFr) console.error(`  - ${k}`);
}
if (missingInEn.length > 0) {
  console.error(`[i18n-parity] Missing in en.json (${missingInEn.length}):`);
  for (const k of missingInEn) console.error(`  - ${k}`);
}
process.exit(1);
