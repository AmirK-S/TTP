// TTP - Talk To Paste
// The current dictation trigger and its label, kept in step with Settings.

import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { safeInvoke } from '../lib/safeInvoke';
import { triggerLabel, type Trigger } from '../lib/trigger';
import { useTauriEvent } from './useTauriEvent';

export function useTrigger() {
  const { t } = useTranslation();
  const [trigger, setTrigger] = useState<Trigger | null>(null);
  const [chars, setChars] = useState('');

  const reload = useCallback(async () => {
    try {
      const next = await safeInvoke<Trigger>('get_trigger');
      setTrigger(next);
      setChars(next.kind === 'key' ? await safeInvoke<string>('trigger_key_chars', { code: next.code }) : '');
    } catch {
      // No backend (tests, plain-browser preview): leave the label empty.
    }
  }, []);

  useEffect(() => { reload(); }, [reload]);
  useTauriEvent('settings-changed', () => { reload(); });

  return { trigger, label: trigger ? triggerLabel(trigger, t, chars) : '', reload };
}
