// TTP - Talk To Paste
// "What's New" modal — shown once after an app update completes. Token-driven,
// blurred backdrop, animated entry — replaces the bare bg-black/50 + bg-blue-600
// modal from before.

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { Sparkles } from 'lucide-react';
import { Button, Modal } from './ui';
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

  // Light markdown: bullets, bold (**...**), inline code (`...`). The changelog
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
      title={t('whatsNew.title', { version: data.version })}
      subtitle={t('whatsNew.subtitle')}
      footer={
        <Button onClick={dismiss} fullWidth size="lg">
          {t('whatsNew.dismiss')}
        </Button>
      }
    >
      <div className="space-y-1.5">
        {data.changelog.split('\n').map(renderLine)}
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
