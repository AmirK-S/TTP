import { describe, it, expect, beforeEach } from 'vitest';
import { followSystemTheme } from './theme';

beforeEach(() => {
  document.documentElement.removeAttribute('data-theme');
  document.documentElement.style.colorScheme = '';
});

describe('followSystemTheme', () => {
  it('drops a forced data-theme left by an older build', () => {
    document.documentElement.setAttribute('data-theme', 'dark');
    followSystemTheme();
    expect(document.documentElement.getAttribute('data-theme')).toBeNull();
  });

  it('sets color-scheme to the OS appearance', () => {
    followSystemTheme();
    expect(['light', 'dark']).toContain(document.documentElement.style.colorScheme);
  });
});
