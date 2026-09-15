// TTP - Talk To Paste
// A dictation trigger and its label, kept in step with Settings. `slot`
// picks the main trigger or the optional second one (null when unset).

import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { safeInvoke } from '../lib/safeInvoke';
import { triggerLabel, type Trigger } from '../lib/trigger';
import { useTauriEvent } from './useTauriEvent';

export type TriggerSlot = 'main' | 'second';

export function useTrigger(slot: TriggerSlot = 'main') {
  const { t } = useTranslation();
  const [trigger, setTrigger] = useState<Trigger | null>(null);
  const [chars, setChars] = useState('');

  const reload = useCallback(async () => {
    try {
      const next = await safeInvoke<Trigger | null>(slot === 'main' ? 'get_trigger' : 'get_secondary_trigger');
      setTrigger(next ?? null);
      setChars(next?.kind === 'key' ? await safeInvoke<string>('trigger_key_chars', { code: next.code }) : '');
    } catch {
      // No backend (tests, plain-browser preview): leave the label empty.
    }
  }, [slot]);

  useEffect(() => { reload(); }, [reload]);
  useTauriEvent('settings-changed', () => { reload(); });

  return { trigger, label: trigger ? triggerLabel(trigger, t, chars) : '', reload };
}
