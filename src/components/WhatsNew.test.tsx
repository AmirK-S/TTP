// TTP - Talk To Paste
// What's New: the changelog now comes from the locale files, not from Rust.
//
// Debt item 3 in docs/overhaul-status.md. `whatsnew.rs` used to hand this
// component a finished string built from ~500 lines of prose compiled into the
// binary, with its own hand-rolled French dispatcher. `check_whats_new` now
// returns a version and nothing else; the text is a translation like every
// other translation in the app.
//
// The case that matters most is the third one. A version with no note must
// render NOTHING and SAY SO — a modal with an empty body would be worse than
// the bug this replaces, and a silent `return null` is how the missing 3.1.7
// note stayed invisible for a whole release in the first place.

import { render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { mockInvoke } from '../test/setup';
import { initI18n, setLanguage } from '../i18n/config';
import WhatsNew, { noteKey } from './WhatsNew';

beforeEach(() => {
  initI18n('en');
  setLanguage('en');
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('the note key', () => {
  it('flattens the version into one i18next path segment', () => {
    // i18next reads `.` as a path separator, so `whatsNew.notes.3.1.7` would
    // address three nested objects that do not exist. `whatsnew.rs` performs
    // the same substitution and has its own test for it.
    expect(noteKey('3.1.7')).toBe('3-1-7');
    expect(noteKey('2.0.2-beta.9')).toBe('2-0-2-beta-9');
  });
});

describe('WhatsNew', () => {
  it('shows nothing when the backend says the note has been seen', async () => {
    mockInvoke.mockResolvedValue(null);
    const { container } = render(<WhatsNew />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('check_whats_new', undefined));
    expect(container).toBeEmptyDOMElement();
  });

  it('renders the note for the version the backend named', async () => {
    mockInvoke.mockImplementation((cmd: string) =>
      cmd === 'check_whats_new' ? Promise.resolve('3.1.7') : Promise.resolve(undefined),
    );
    render(<WhatsNew />);
    expect(await screen.findByText(/What's new in v3\.1\.7/)).toBeInTheDocument();
    // A line from the middle of the real 3.1.7 entry, so this fails if the
    // body is empty, truncated, or the wrong version's.
    expect(screen.getByText(/the pill has a face/)).toBeInTheDocument();
  });

  it('renders the French note when the app is in French', async () => {
    setLanguage('fr');
    mockInvoke.mockImplementation((cmd: string) =>
      cmd === 'check_whats_new' ? Promise.resolve('3.1.7') : Promise.resolve(undefined),
    );
    render(<WhatsNew />);
    // The old Rust dispatcher had to be told about each French entry by hand
    // and could fall through to English. i18next just resolves the key.
    expect(await screen.findByText(/la pilule a un visage/)).toBeInTheDocument();
    // `beforeEach` puts it back; switching here would re-render outside act().
  });

  it('renders nothing, loudly, for a version with no note', async () => {
    const err = vi.spyOn(console, 'error').mockImplementation(() => {});
    mockInvoke.mockImplementation((cmd: string) =>
      cmd === 'check_whats_new' ? Promise.resolve('9.9.9') : Promise.resolve(undefined),
    );
    const { container } = render(<WhatsNew />);
    await waitFor(() => expect(err).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
    // The message has to name the version and the key, or whoever reads it
    // still has to go looking.
    const said = err.mock.calls.flat().join(' ');
    expect(said).toContain('9.9.9');
    expect(said).toContain('whatsNew.notes.9-9-9');
  });

  it('does not dismiss a version it could not show', async () => {
    // Dismissing would write `last_seen_version`, and the note would then be
    // skipped forever even after somebody added it.
    vi.spyOn(console, 'error').mockImplementation(() => {});
    mockInvoke.mockImplementation((cmd: string) =>
      cmd === 'check_whats_new' ? Promise.resolve('9.9.9') : Promise.resolve(undefined),
    );
    render(<WhatsNew />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());
    expect(mockInvoke).not.toHaveBeenCalledWith('dismiss_whats_new', expect.anything());
  });

  it('renders bullets as bullets and keeps the leading paragraph plain', async () => {
    mockInvoke.mockImplementation((cmd: string) =>
      cmd === 'check_whats_new' ? Promise.resolve('3.1.7') : Promise.resolve(undefined),
    );
    render(<WhatsNew />);
    await screen.findByText(/TTP is free\./);
    // The `• ` prefix is markup, not text: it must not survive into the DOM.
    expect(screen.queryByText(/^•/)).toBeNull();
  });
});
