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

/** Read leaf string for a dot-separated key path. Returns undefined if not a string. */
function leafString(obj, dottedKey) {
  const parts = dottedKey.split('.');
  let cur = obj;
  for (const p of parts) {
    if (cur == null || typeof cur !== 'object') return undefined;
    cur = cur[p];
  }
  return typeof cur === 'string' ? cur : undefined;
}

// Strings that are intentionally identical across locales (product names, dev
// tokens, placeholders, the literal Groq dictionary example, brand handles).
// Keep this list tight — every entry is a future excuse for an untranslated
// menu item to slip through.
const ALLOW_IDENTICAL = new Set([
  // Product / brand names
  'tray.tooltip',
  'setup.title',
  'notification.appName',
  'windowTitle.main',
  'settings.pro.title',
  'settings.pro.badgeActive',
  'settings.nav.pro',
  'settings.about.subtitle',
  // Apple / OS UI labels we keep as-is in French System Settings UX
  'onboarding.item.microphone',
  'onboarding.tour.spotlightHint',
  // Cognates that are literally the same word in FR/EN
  'common.active',
  'settings.dictionary.labelCorrection',
  'settings.dictionary.tableCorrection',
  'settings.dictionary.tableOriginal',
  'settings.transcription.title',
  'settings.nav.capture',
  'settings.updateChannel.labelStable',
  'settings.logs.title',
  'settings.pro.labelActivations',
  'settings.analytics.transcriptions',
  'settings.sidebar.transcriptions',
  // Language self-references
  'settings.language.optionEnglish',
  'settings.language.optionFrench',
  // Shortcut renderings, placeholders, pure symbols
  'settings.recordingTrigger.optionCmdShiftR',
  'settings.recordingTrigger.optionFn',
  'settings.recordingTrigger.optionWinJ',
  'settings.transcription.keyPlaceholder',
  'form.apiKey.placeholder',
  'settings.dictionary.placeholderMisheard',
  'settings.dictionary.placeholderCorrection',
  'onboarding.button.checking',
]);

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

// Catch leaves that match EN verbatim in FR — this is how v2.x shipped a
// French tray menu reading "Fix Accessibility permission" for two minor
// versions. The set above carves out legitimate exceptions (product names,
// placeholders). Anything else identical across locales is a missed translation.
const sharedKeys = [...enSet].filter((k) => frSet.has(k));
const identicalLeaves = sharedKeys
  .filter((k) => !ALLOW_IDENTICAL.has(k))
  .filter((k) => {
    const e = leafString(en, k);
    const f = leafString(fr, k);
    if (typeof e !== 'string' || typeof f !== 'string') return false;
    // Skip empty strings (deliberately blank, e.g. mid-pipeline message slots).
    if (!e.trim() || !f.trim()) return false;
    // Skip strings that are pure interpolation placeholders or numbers.
    if (/^[\s{}\w.]*$/.test(e) && /^\{\{[^}]+\}\}$/.test(e.trim())) return false;
    return e === f;
  })
  .sort();

const ok = missingInFr.length === 0 && missingInEn.length === 0 && identicalLeaves.length === 0;

if (ok) {
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
if (identicalLeaves.length > 0) {
  console.error(`[i18n-parity] FR matches EN verbatim (likely untranslated, ${identicalLeaves.length}):`);
  for (const k of identicalLeaves) {
    console.error(`  - ${k}: "${leafString(en, k)}"`);
  }
  console.error(`[i18n-parity] If a string is intentionally identical (brand, placeholder), add its key to ALLOW_IDENTICAL in this script.`);
}
process.exit(1);
