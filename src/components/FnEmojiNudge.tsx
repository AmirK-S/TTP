import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useTranslation } from 'react-i18next';
import { ExternalLink } from 'lucide-react';
import { Banner, Button } from './ui';
import { useSettingsStore } from '../stores/settings-store';

const IS_MAC = typeof navigator !== 'undefined' && navigator.platform.startsWith('Mac');

/**
 * macOS only. When the recording trigger is the Fn/🌐 key but the Globe key is
 * still configured to open the emoji picker (or dictation / input-source
 * switch), every Fn press pops that system UI instead of just recording. We
 * can't flip that setting programmatically with live effect, so we guide the
 * user to System Settings → Keyboard → "Press 🌐 key to → Do Nothing". The Fn
 * recording keeps working regardless of this setting. Auto-dismisses once the
 * user has changed it: re-checks when the window regains focus (covers popping
 * out to System Settings and back) plus a short interval backstop.
 */
export function FnEmojiNudge() {
  const { t } = useTranslation();
  const shortcut = useSettingsStore((s) => s.shortcut);
  const [intercepts, setIntercepts] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setIntercepts(await invoke<boolean>('fn_globe_key_intercepts'));
    } catch (e) {
      console.error('FnEmojiNudge: check failed', e);
    }
  }, []);

  useEffect(() => {
    if (!IS_MAC || shortcut !== 'FnKey') return;
    refresh();
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) refresh();
    });
    const id = window.setInterval(refresh, 2000);
    return () => {
      unlisten.then((fn) => fn());
      window.clearInterval(id);
    };
  }, [refresh, shortcut]);

  if (!IS_MAC || shortcut !== 'FnKey' || !intercepts) return null;

  return (
    <Banner
      tone="warning"
      title={t('settings.recordingTrigger.fnEmojiTitle')}
      action={
        <Button
          size="sm"
          variant="secondary"
          onClick={() => invoke('open_keyboard_settings').catch(console.error)}
          leftIcon={<ExternalLink className="size-3" />}
        >
          {t('settings.recordingTrigger.fnEmojiButton')}
        </Button>
      }
      className="mt-4"
    >
      {t('settings.recordingTrigger.fnEmojiDesc')}
    </Banner>
  );
}
