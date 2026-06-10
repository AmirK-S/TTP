import { describe, it, expect, beforeEach, vi } from 'vitest';
import { resolveLanguage } from './config';

describe('resolveLanguage', () => {
  beforeEach(() => {
    // Reset navigator.language stubs between tests.
    vi.unstubAllGlobals();
  });

  it('returns the explicit choice when en or fr', () => {
    expect(resolveLanguage('en')).toBe('en');
    expect(resolveLanguage('fr')).toBe('fr');
  });

  it('falls back to fr when system + navigator.language starts with fr', () => {
    vi.stubGlobal('navigator', { language: 'fr-FR' });
    expect(resolveLanguage('system')).toBe('fr');
  });

  it('falls back to fr when navigator.language is bare "fr"', () => {
    vi.stubGlobal('navigator', { language: 'fr' });
    expect(resolveLanguage('system')).toBe('fr');
  });

  it('falls back to en when system + navigator.language starts with en', () => {
    vi.stubGlobal('navigator', { language: 'en-US' });
    expect(resolveLanguage('system')).toBe('en');
  });

  it('falls back to en for unknown navigator locales', () => {
    vi.stubGlobal('navigator', { language: 'de-DE' });
    expect(resolveLanguage('system')).toBe('en');
    vi.stubGlobal('navigator', { language: 'ja-JP' });
    expect(resolveLanguage('system')).toBe('en');
  });

  it('treats null and undefined as system', () => {
    vi.stubGlobal('navigator', { language: 'fr-CA' });
    expect(resolveLanguage(null)).toBe('fr');
    expect(resolveLanguage(undefined)).toBe('fr');
  });

  it('is case-insensitive on the locale prefix', () => {
    vi.stubGlobal('navigator', { language: 'FR-CA' });
    expect(resolveLanguage(null)).toBe('fr');
  });
});
