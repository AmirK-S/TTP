import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useTranslation } from 'react-i18next';
import { Banner, Button } from './ui';

type PermissionStatus = 'Granted' | 'Denied' | 'Undetermined';
type PermKey = 'accessibility' | 'microphone' | 'inputMonitoring';

const IS_MAC = typeof navigator !== 'undefined' && navigator.platform.startsWith('Mac');

const OPEN_COMMAND: Record<PermKey, string> = {
  accessibility: 'open_accessibility_settings',
  microphone: 'open_microphone_settings',
  inputMonitoring: 'open_input_monitoring_settings',
};

/**
 * Inline permission warning. Polls the relevant Rust permission checks on
 * mount and whenever the host window regains focus (covers the user popping
 * out to System Settings and back). Renders nothing when everything's fine.
 *
 * Designed to be dropped at the top of any window (Settings is the obvious
 * host). Accessibility + Microphone are the only deal-breakers; Input
 * Monitoring is conditional on the Fn-key flow being active, so we only
 * surface it when the setting is on.
 */
export function PermissionBanner() {
  const { t } = useTranslation();
  const [missing, setMissing] = useState<PermKey[]>([]);

  const refresh = useCallback(async () => {
    if (!IS_MAC) {
      setMissing([]);
      return;
    }
    try {
      const [mic, ax, im, settings] = await Promise.all([
        invoke<PermissionStatus>('check_microphone_permission'),
        invoke<PermissionStatus>('check_accessibility_permission'),
        invoke<boolean>('check_input_monitoring_permission'),
        invoke<{ fn_key_enabled?: boolean }>('get_settings').catch(() => ({ fn_key_enabled: false })),
      ]);
      const out: PermKey[] = [];
      if (mic !== 'Granted') out.push('microphone');
      if (ax !== 'Granted') out.push('accessibility');
      // Only flag Input Monitoring when the user actually depends on Fn —
      // otherwise it's noise (other hotkeys don't need this permission).
      if (settings?.fn_key_enabled && !im) out.push('inputMonitoring');
      setMissing(out);
    } catch (e) {
      console.error('PermissionBanner: refresh failed', e);
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) refresh();
    });
    return () => { unlisten.then((fn) => fn()); };
  }, [refresh]);

  if (missing.length === 0) return null;

  const labels = missing.map((k) => t(`onboarding.item.${k}`));
  const title = missing.length === 1
    ? t('permissionBanner.titleOne', { item: labels[0] })
    : t('permissionBanner.titleMany');

  return (
    <Banner
      tone="warning"
      title={title}
      action={
        missing.length === 1 ? (
          <Button size="sm" variant="secondary" onClick={() => invoke(OPEN_COMMAND[missing[0]])}>
            {t('onboarding.button.openSettings')}
          </Button>
        ) : undefined
      }
      className="mb-4"
    >
      {missing.length > 1 && (
        <div className="mt-2 flex flex-wrap gap-2">
          {missing.map((k) => (
            <Button
              key={k}
              size="sm"
              variant="secondary"
              onClick={() => invoke(OPEN_COMMAND[k])}
            >
              {t(`onboarding.item.${k}`)}
            </Button>
          ))}
        </div>
      )}
    </Banner>
  );
}
