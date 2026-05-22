// TTP - Talk To Paste
// "What's New" modal — shown once after an app update completes. Token-driven,
// blurred backdrop, animated entry — replaces the bare bg-black/50 + bg-blue-600
// modal from before.

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { Sparkles } from 'lucide-react';
import { Button } from './ui';
import { cn } from '../lib/cn';

interface WhatsNewData {
  version: string;
  changelog: string;
}

export default function WhatsNew() {
  const { t } = useTranslation();
  const [data, setData] = useState<WhatsNewData | null>(null);

  useEffect(() => {
    invoke<[string, string] | null>('check_whats_new')
      .then((result) => { if (result) setData({ version: result[0], changelog: result[1] }); })
      .catch((err) => console.error('[WhatsNew] Failed to check:', err));
  }, []);

  if (!data) return null;

  const dismiss = async () => {
    try { await invoke('dismiss_whats_new'); }
    catch (err) { console.error('[WhatsNew] Failed to dismiss:', err); }
    setData(null);
  };

  // Light markdown — bullets, bold (**...**), inline code (`...`). The changelog
  // is authored by us in Rust as plain text with `- ` bullets; we render it
  // semantically rather than dumping raw lines.
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
    <div
      className={cn(
        'fixed inset-0 z-50 flex items-center justify-center p-6',
        'bg-black/40 backdrop-blur-md anim-fade-in',
      )}
      role="dialog"
      aria-modal="true"
      aria-labelledby="whatsnew-title"
    >
      <div
        className={cn(
          'anim-scale-in bg-app-surface text-app-text rounded-app-xl shine-md border border-app-border',
          'max-w-md w-full max-h-[85vh] flex flex-col',
        )}
      >
        <div className="px-6 pt-6 pb-4 shrink-0 flex items-start gap-4">
          <div className="size-10 rounded-app-md bg-app-accent-tint grid place-items-center shrink-0">
            <Sparkles className="size-5 text-app-accent" aria-hidden />
          </div>
          <div className="min-w-0">
            <h2 id="whatsnew-title" className="text-display-xs text-app-text">
              {t('whatsNew.title', { version: data.version })}
            </h2>
            <p className="mt-1 text-[12px] text-app-muted">{t('whatsNew.subtitle')}</p>
          </div>
        </div>

        <div className="px-6 space-y-1.5 overflow-y-auto flex-1 min-h-0">
          {data.changelog.split('\n').map(renderLine)}
        </div>

        <div className="px-6 pt-5 pb-6 shrink-0">
          <Button onClick={dismiss} fullWidth size="lg">
            {t('whatsNew.dismiss')}
          </Button>
        </div>
      </div>
    </div>
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
