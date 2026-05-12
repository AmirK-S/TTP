// TTP - Talk To Paste
// i18next initialization
//
// Languages:
//  - 'en' : English (fallback)
//  - 'fr' : French
//  - 'system' : follow navigator.language at startup (resolved to en or fr)
//
// The user choice is persisted in Settings.language as 'en' | 'fr' | 'system'.
// When 'system' or null, we resolve to fr if navigator.language starts with "fr",
// otherwise en.

import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

import en from './locales/en.json';
import fr from './locales/fr.json';

export type LanguageChoice = 'en' | 'fr' | 'system';

/** Resolve a stored language choice to a concrete locale ('en' | 'fr'). */
export function resolveLanguage(choice: LanguageChoice | null | undefined): 'en' | 'fr' {
  if (choice === 'fr' || choice === 'en') return choice;
  // 'system' or null/undefined → autodetect from navigator
  const nav = typeof navigator !== 'undefined' ? navigator.language : '';
  return nav.toLowerCase().startsWith('fr') ? 'fr' : 'en';
}

/** Initialize i18next. Idempotent — safe to call multiple times. */
export function initI18n(initialLanguage: LanguageChoice | null = null): typeof i18n {
  if (i18n.isInitialized) return i18n;

  const lng = resolveLanguage(initialLanguage);

  i18n
    .use(initReactI18next)
    .init({
      resources: {
        en: { translation: en },
        fr: { translation: fr },
      },
      lng,
      fallbackLng: 'en',
      interpolation: { escapeValue: false }, // React already escapes
      returnEmptyString: false,
    });

  return i18n;
}

/** Change the runtime language. Accepts the raw user choice ('system' is resolved). */
export function setLanguage(choice: LanguageChoice | null): void {
  const resolved = resolveLanguage(choice);
  if (i18n.language !== resolved) {
    i18n.changeLanguage(resolved);
  }
}

export default i18n;
