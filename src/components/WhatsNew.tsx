// TTP - Talk To Paste
// "What's New" modal — shown once after an app update completes. Token-driven,
// blurred backdrop, animated entry.
//
// The changelog text is a TRANSLATION, read from `whatsNew.notes.<version>` in
// the locale files. It used to arrive from Rust as a finished string, which
// meant ~500 lines of user-facing prose lived in `whatsnew.rs` behind a
// hand-written French dispatcher that could silently serve English — the
// standing "no user-facing prose in Rust" rule, broken by every entry rather
// than by one. `check_whats_new` now answers the only question Rust can
// answer: which version is running, and has this user seen its note.

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { Sparkles } from 'lucide-react';
import { Button, Modal } from './ui';
import { cn } from '../lib/cn';

/**
 * The i18next key segment a version's note lives under.
 *
 * i18next reads `.` as a path separator, so `whatsNew.notes.3.1.7` would
 * address three nested objects instead of one entry. `whatsnew.rs::note_key`
 * performs the same substitution and has its own test; if either side changes
 * it, one of the two goes red.
 */
export function noteKey(version: string): string {
  return version.replace(/\./g, '-');
}

export default function WhatsNew() {
  const { t, i18n } = useTranslation();
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    invoke<string | null>('check_whats_new')
      .then((result) => { if (result) setVersion(result); })
      .catch((err) => console.error('[WhatsNew] Failed to check:', err));
  }, []);

  const key = version ? `whatsNew.notes.${noteKey(version)}` : null;
  // `exists` walks the fallback chain, so a note that is in en.json but not
  // yet in fr.json still shows (in English) rather than vanishing.
  const hasNote = key !== null && i18n.exists(key);

  useEffect(() => {
    if (!version || hasNote) return;
    // Say it out loud. The 3.1.7 release shipped with no note at all and
    // nobody noticed for a full version, because the missing case rendered
    // nothing and reported nothing — indistinguishable from "already seen".
    // `whatsnew.rs`'s guard test now fails the build on this, and if one ever
    // gets past it, the console names the version and the key.
    console.error(
      `[WhatsNew] v${version} has no changelog entry — add \`whatsNew.notes.${noteKey(version)}\`` +
      ' to src/i18n/locales/en.json and fr.json',
    );
  }, [version, hasNote]);

  // Nothing to show, and deliberately NOT dismissed: writing
  // `last_seen_version` here would skip the note forever, including after
  // somebody adds it.
  if (!version || !key || !hasNote) return null;

  const changelog = t(key);

  const dismiss = async () => {
    try { await invoke('dismiss_whats_new'); }
    catch (err) { console.error('[WhatsNew] Failed to dismiss:', err); }
    setVersion(null);
  };

  // Light markdown: bullets, bold (**...**), inline code (`...`). The note is
  // authored in the locale files as plain text whose lines start with `• `;
  // we render it semantically rather than dumping raw lines.
  const renderLine = (line: string, i: number) => {
    const isBullet = /^\s*[-*•]\s+/.test(line);
    const clean = isBullet ? line.replace(/^\s*[-*•]\s+/, '') : line;
    if (!clean.trim()) return <div key={i} className="h-1" aria-hidden />;
    return (
      <div key={i} className={cn('flex gap-2.5 text-[13px] leading-relaxed', !isBullet && 'pl-4')}>
        {isBullet && (
          <span className="mt-[7px] size-1 rounded-full bg-app-accent shrink-0" aria-hidden />
        )}
        <span className="text-app-muted">{renderInline(clean)}</span>
      </div>
    );
  };

  return (
    <Modal
      open
      onClose={dismiss}
      size="md"
      scrollableContent
      closeOnBackdrop={false}
      headerLeading={
        <div className="size-10 rounded-app-md bg-app-accent-tint grid place-items-center">
          <Sparkles className="size-5 text-app-accent" aria-hidden />
        </div>
      }
      title={t('whatsNew.title', { version })}
      subtitle={t('whatsNew.subtitle')}
      footer={
        <Button onClick={dismiss} fullWidth size="lg">
          {t('whatsNew.dismiss')}
        </Button>
      }
    >
      <div className="space-y-1.5">
        {changelog.split('\n').map(renderLine)}
      </div>
    </Modal>
  );
}

/** Renders inline markdown bits: `**bold**`, `` `code` ``. Skips anything else. */
function renderInline(text: string): React.ReactNode {
  const parts: React.ReactNode[] = [];
  const regex = /(\*\*[^*]+\*\*|`[^`]+`)/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = regex.exec(text)) !== null) {
    if (m.index > last) parts.push(text.slice(last, m.index));
    const token = m[0];
    if (token.startsWith('**')) {
      parts.push(<strong key={m.index} className="font-medium text-app-text">{token.slice(2, -2)}</strong>);
    } else {
      parts.push(
        <code
          key={m.index}
          className="px-1 py-0.5 rounded-app-sm bg-app-raised font-mono text-[11px] text-app-text"
        >
          {token.slice(1, -1)}
        </code>
      );
    }
    last = m.index + token.length;
  }
  if (last < text.length) parts.push(text.slice(last));
  return parts;
}
