import { ui, defaultLang, type Lang } from './ui';

export type { Lang };

export function getLangFromUrl(url: URL): Lang {
  const [, lang] = url.pathname.split('/');
  if (lang in ui) return lang as Lang;
  return defaultLang;
}

export function useTranslations(lang: Lang) {
  return function t(key: keyof (typeof ui)[typeof defaultLang]): string {
    return ui[lang][key] || ui[defaultLang][key];
  };
}

export function getLocalizedPath(lang: Lang, anchor: string = ''): string {
  const prefix = lang === defaultLang ? '' : `/${lang}`;
  return `${prefix}/${anchor}`;
}

/**
 * Pairs of routes that mean the same page in the two languages.
 *
 * The manual's route is translated because the page is prose, and a French
 * reader arriving at `/fr/manual` would be reading a French page at an English
 * address. The legal pages keep their slugs — they are linked from elsewhere
 * and the words are the same either way.
 */
const ROUTE_PAIRS: ReadonlyArray<readonly [string, string]> = [
  ['/', '/fr/'],
  ['/manual', '/fr/manuel'],
  ['/privacy', '/fr/privacy'],
  ['/terms', '/fr/terms'],
];

/** The current page's address in the other language, or that language's home. */
export function otherLangPath(url: URL, target: Lang): string {
  const path = url.pathname.replace(/\/index\.html$/, '/');
  const index = target === 'fr' ? 1 : 0;
  for (const pair of ROUTE_PAIRS) {
    if (pair[0] === path || pair[1] === path) return pair[index];
  }
  return target === 'fr' ? '/fr/' : '/';
}
