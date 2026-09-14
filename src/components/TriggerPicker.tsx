// TTP - Talk To Paste
// Settings → Trigger, on macOS: "press the key you want".
//
// The capture itself happens in Rust (the event tap is the only thing that can
// see Fn, a single side of a modifier, or a mouse button); this component
// starts it, shows what came back, and saves it.

import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { ExternalLink } from 'lucide-react';
import { Button } from './ui';
import { useTauriEvent } from '../hooks/useTauriEvent';
import { useTrigger } from '../hooks/useTrigger';
import { cn } from '../lib/cn';
import { triggerLabel, type CaptureResult, type Trigger } from '../lib/trigger';

/** One-click choices for the triggers that are awkward to press on purpose
 *  (Fn opens the emoji picker the moment it is tapped). */
const PRESETS: Trigger[] = [
  { kind: 'fn' },
  { kind: 'modifier', code: 54 }, // right command
  { kind: 'modifier', code: 61 }, // right option
];

export function TriggerPicker() {
  const { t } = useTranslation();
  const { trigger, label, reload } = useTrigger();
  const [capturing, setCapturing] = useState(false);
  const [message, setMessage] = useState<{ tone: 'error' | 'success'; text: string } | null>(null);
  const [inputMonitoringMissing, setInputMonitoringMissing] = useState(false);
  const capturingRef = useRef(false);

  useEffect(() => {
    invoke<boolean>('check_input_monitoring')
      .then((granted) => setInputMonitoringMissing(!granted))
      .catch(() => {});
  }, []);

  // A capture swallows every key system-wide; never leave one running behind
  // a closed window. Rust also times out on its own after 15 s.
  useEffect(() => () => {
    if (capturingRef.current) invoke('cancel_trigger_capture').catch(() => {});
  }, []);

  const save = useCallback(async (next: Trigger) => {
    try {
      await invoke('set_trigger', { trigger: next });
      await reload();
      setMessage({ tone: 'success', text: t('settings.recordingTrigger.successUpdated') });
      setTimeout(() => setMessage(null), 3000);
    } catch (e) {
      const key = String(e);
      setMessage({ tone: 'error', text: key.startsWith('error.') ? t(key) : key });
    }
  }, [reload, t]);

  const startCapture = async () => {
    setMessage(null);
    try {
      await invoke('start_trigger_capture');
      capturingRef.current = true;
      setCapturing(true);
    } catch (e) {
      setMessage({ tone: 'error', text: String(e) });
    }
  };

  const stopCapture = () => {
    capturingRef.current = false;
    setCapturing(false);
  };

  useTauriEvent<CaptureResult>('trigger-captured', (event) => {
    // Every open window with a picker hears the event (Settings and
    // onboarding can both be up); only the one that started the capture acts,
    // or the trigger is saved twice.
    if (!capturingRef.current) return;
    const result = event.payload;
    if (result.status === 'rejected') {
      // Still listening: say why and let them press something else.
      setMessage({ tone: 'error', text: t(`error.trigger_${result.reason}`) });
      return;
    }
    stopCapture();
    if (result.status === 'captured') void save(result.trigger);
  });

  const cancelCapture = () => {
    invoke('cancel_trigger_capture').catch(() => {});
    stopCapture();
  };

  return (
    <div>
      <div
        className={cn(
          'flex items-center justify-between gap-4 rounded-app-md border px-4 py-3',
          capturing ? 'border-app-accent bg-app-accent-tint' : 'border-app-border bg-app-surface',
        )}
        aria-live="polite"
      >
        {capturing ? (
          <p className="text-[13px] text-app-text">{t('settings.recordingTrigger.capturePrompt')}</p>
        ) : (
          <kbd className="inline-flex items-center min-h-[28px] px-2.5 rounded-app-sm bg-app-raised border border-app-border-strong text-[13px] font-mono font-semibold text-app-text">
            {label || '…'}
          </kbd>
        )}
        {capturing ? (
          <Button variant="ghost" size="sm" onClick={cancelCapture}>{t('common.cancel')}</Button>
        ) : (
          <Button variant="secondary" size="sm" onClick={startCapture}>{t('common.change')}</Button>
        )}
      </div>

      {!capturing && (
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <span className="text-[12px] text-app-muted">{t('settings.recordingTrigger.presets')}</span>
          {PRESETS.map((preset) => {
            const selected = trigger && JSON.stringify(trigger) === JSON.stringify(preset);
            return (
              <button
                key={JSON.stringify(preset)}
                type="button"
                onClick={() => save(preset)}
                disabled={Boolean(selected)}
                className={cn(
                  'px-2 py-1 rounded-app-sm border text-[12px] font-mono transition-colors',
                  selected
                    ? 'border-app-accent text-app-accent cursor-default'
                    : 'border-app-border text-app-muted hover:text-app-text hover:bg-app-raised',
                )}
              >
                {triggerLabel(preset, t)}
              </button>
            );
          })}
        </div>
      )}

      {trigger?.kind === 'modifier' && !capturing && (
        <p className="mt-3 text-[12px] text-app-muted leading-relaxed">{t('settings.recordingTrigger.modifierHint')}</p>
      )}

      {message && (
        <p className={cn('mt-3 text-[13px]', message.tone === 'error' ? 'text-app-danger' : 'text-app-success')}>
          {message.text}
        </p>
      )}

      {inputMonitoringMissing && (
        <div className="mt-4 space-y-2">
          <p className="text-[13px] text-app-danger">{t('error.input_monitoring_required')}</p>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => invoke('open_input_monitoring_settings').catch(console.error)}
            leftIcon={<ExternalLink className="size-3" />}
          >
            {t('settings.recordingTrigger.openInputMonitoring')}
          </Button>
        </div>
      )}
    </div>
  );
}
