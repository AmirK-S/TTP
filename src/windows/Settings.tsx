// TTP - Talk To Paste
// Settings window — configure app behavior, manage dictionary/history, manage
// the Pro license. Five IA groups: General / Capture / Pro / Data / Advanced.

import { useEffect, useState, useCallback, useRef, memo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Copy, Check, Download, RefreshCw, Crown, ArrowRight, ExternalLink,
  User, Mic, SlidersHorizontal, Database, BookOpen, Repeat,
} from 'lucide-react';
import { cn } from '../lib/cn';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useTauriEvent } from '../hooks/useTauriEvent';
import { getVersion } from '@tauri-apps/api/app';
import { relaunch } from '@tauri-apps/plugin-process';
import { enable as enableAutostart, disable as disableAutostart } from '@tauri-apps/plugin-autostart';
import { trackEvent } from '../lib/analytics';
import { useUpdater } from '../hooks/useUpdater';
import { useSettingsStore, DictionaryEntry, HistoryEntry } from '../stores/settings-store';
import { PermissionBanner } from '../components/PermissionBanner';
import { FnEmojiNudge } from '../components/FnEmojiNudge';
import type { LanguageChoice } from '../i18n/config';
import type { ThemeChoice } from '../lib/theme';
import WhatsNew from '../components/WhatsNew';
import {
  Button, Input, Banner, Spinner, Toggle, ConfirmDialog,
  EmptyState, SettingsSection, SettingsRow, RadioOption, BrandTile,
} from '../components/ui';

/* ----------------------------------------------------------------------------
   Small leaf components
   ------------------------------------------------------------------------- */

function formatTimestamp(timestamp: number, locale: string): string {
  return new Date(timestamp).toLocaleString(locale, {
    month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit', hour12: true,
  });
}

/**
 * Dictionary table row — stable identity for memo via the `original` key.
 */
const DictionaryRow = memo(function DictionaryRow({
  entry, onDelete,
}: {
  entry: DictionaryEntry;
  onDelete: (original: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="group flex items-center gap-3 px-4 py-2.5 hover:bg-app-raised transition-colors duration-hover">
      <div className="flex-1 min-w-0 grid grid-cols-2 gap-3 items-center">
        <code className="text-[12px] text-app-muted font-mono truncate">{entry.original}</code>
        <code className="text-[12px] text-app-text font-mono truncate">{entry.correction}</code>
      </div>
      <Button
        variant="ghost"
        size="sm"
        onClick={() => onDelete(entry.original)}
        className="opacity-0 group-hover:opacity-100 text-app-danger hover:text-app-danger hover:bg-app-danger-tint"
      >
        {t('common.delete')}
      </Button>
    </div>
  );
});

const HistoryRow = memo(function HistoryRow({ entry }: { entry: HistoryEntry }) {
  const { t, i18n } = useTranslation();
  const [copied, setCopied] = useState(false);
  const [replaying, setReplaying] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(entry.text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (error) { console.error('Failed to copy:', error); }
  };

  const handleReplay = async () => {
    if (replaying) return;
    setReplaying(true);
    try {
      // Close the Settings window first so the focused-app target is
      // whatever the user was working on, not the Settings window itself.
      // Replay completes asynchronously on the Rust side after the window
      // is gone — accessibility events go to the newly-focused app.
      await getCurrentWindow().hide();
      await new Promise((r) => setTimeout(r, 120));
      await invoke('replay_history_entry', { text: entry.text });
    } catch (error) {
      console.error('Failed to replay history entry:', error);
    } finally {
      setReplaying(false);
    }
  };

  const preview = entry.text.length > 100 ? entry.text.slice(0, 100) + '…' : entry.text;

  return (
    <div className="group flex items-start gap-3 px-4 py-3 hover:bg-app-raised transition-colors duration-hover">
      <div className="flex-1 min-w-0">
        <p className="text-[11px] text-app-faint mb-1 tabular-nums">{formatTimestamp(entry.timestamp, i18n.language)}</p>
        <p className="text-[13px] text-app-text break-words leading-relaxed">{preview}</p>
      </div>
      <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 shrink-0">
        <Button
          variant="ghost"
          size="sm"
          onClick={handleReplay}
          loading={replaying}
          title={t('settings.history.replayTooltip')}
        >
          <Repeat className="size-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={handleCopy}
          title={t('settings.dictionary.copyTooltip')}
        >
          {copied ? <Check className="size-3.5 text-app-success" /> : <Copy className="size-3.5" />}
        </Button>
      </div>
    </div>
  );
});

/* ----------------------------------------------------------------------------
   Analytics — user's own usage stats. Local-only.
   ------------------------------------------------------------------------- */

interface AnalyticsWindowData { transcriptions: number; words: number; chars: number; }
interface AnalyticsSummary {
  week: AnalyticsWindowData;
  month: AnalyticsWindowData;
  all_time: AnalyticsWindowData;
  daily: Array<{ date: string; transcriptions: number; words: number; chars: number }>;
}

/* The full analytics card (3-column week/month/all-time) was removed from the
   Pro section in v2.1.3 — user feedback said it sat at the bottom of the
   scroll where it never got read. SidebarStats (at the bottom of the sidebar)
   shows a sleek one-line summary that's always visible instead. Type kept
   above so SidebarStats can reuse the IPC return shape. */

/* ----------------------------------------------------------------------------
   Update channel (beta opt-in)
   ------------------------------------------------------------------------- */

function UpdateChannelCard() {
  const { t } = useTranslation();
  const { useBetaChannel, saveSettings, loading } = useSettingsStore();
  const [appVersion, setAppVersion] = useState('...');
  const [pendingDowngrade, setPendingDowngrade] = useState(false);

  useEffect(() => { getVersion().then(setAppVersion).catch(() => {}); }, []);

  const isOnBetaBuild = appVersion.includes('-beta');

  const persist = async (enabled: boolean) => {
    try {
      await saveSettings({ use_beta_channel: enabled });
      trackEvent('setting_changed', { setting_name: 'use_beta_channel', new_value: String(enabled) });
    } catch (error) { console.error('Failed to save update channel setting:', error); }
  };

  const handleToggle = (enabled: boolean) => {
    if (!enabled && isOnBetaBuild) { setPendingDowngrade(true); return; }
    persist(enabled);
  };

  return (
    <SettingsSection
      title={t('settings.updateChannel.title')}
      action={isOnBetaBuild ? (
        <span className="px-2 py-0.5 text-[11px] font-semibold rounded-full bg-app-accent-tint text-app-accent">
          {t('settings.updateChannel.betaBadge')}
        </span>
      ) : undefined}
    >
      <SettingsRow
        label={useBetaChannel ? t('settings.updateChannel.labelBeta') : t('settings.updateChannel.labelStable')}
        description={useBetaChannel ? t('settings.updateChannel.descBeta') : t('settings.updateChannel.descStable')}
        control={<Toggle enabled={useBetaChannel} onChange={handleToggle} disabled={loading} />}
      />
      {useBetaChannel && (
        <Banner tone="warning" className="mt-4">{t('settings.updateChannel.warning')}</Banner>
      )}
      <ConfirmDialog
        open={pendingDowngrade}
        title={t('dialog.downgradeBeta.title')}
        message={t('dialog.downgradeBeta.message')}
        confirmText={t('common.continue')}
        cancelText={t('common.cancel')}
        tone="warning"
        onConfirm={() => { setPendingDowngrade(false); persist(false); }}
        onCancel={() => setPendingDowngrade(false)}
      />
    </SettingsSection>
  );
}

/* ----------------------------------------------------------------------------
   Updates (check-for-update)
   ------------------------------------------------------------------------- */

function UpdatesCard() {
  const { t } = useTranslation();
  const { status, updateInfo, progress, error, checkForUpdates, downloadAndInstall, restartApp, dismiss } = useUpdater();
  const [appVersion, setAppVersion] = useState('...');
  const [buildSha, setBuildSha] = useState<string>('');
  const [channel, setChannel] = useState<string>('stable');

  useEffect(() => { getVersion().then(setAppVersion).catch(() => {}); }, []);
  useEffect(() => {
    // Build info: marketing version + git SHA + channel. The SHA is the
    // ground truth when the user is on a beta — marketing version stays
    // "3.0.0" across betas since the Windows MSI bundler rejects non-numeric
    // pre-release suffixes.
    invoke<{ version: string; commit_sha: string; channel: string }>('get_build_info')
      .then((info) => {
        setBuildSha(info.commit_sha);
        setChannel(info.channel);
      })
      .catch(() => {});
  }, []);
  useTauriEvent('update-available', () => { checkForUpdates(); });

  const versionLabel = (
    <span className="text-[11px] text-app-faint tabular-nums">
      v{appVersion}
      {buildSha && buildSha !== 'unknown' ? ` · ${buildSha}` : ''}
      {channel === 'beta' ? ' · beta' : ''}
    </span>
  );

  return (
    <SettingsSection
      title={t('settings.updates.title')}
      action={versionLabel}
    >
      <div className="space-y-3">
        {status === 'idle' && (
          <Button variant="secondary" size="md" onClick={checkForUpdates} leftIcon={<RefreshCw className="size-3.5" />}>
            {t('settings.updates.checkButton')}
          </Button>
        )}
        {status === 'up-to-date' && (
          <div className="flex items-center gap-2 text-[13px] text-app-success">
            <Check className="size-4" /> {t('settings.updates.upToDate', { version: appVersion })}
          </div>
        )}
        {status === 'checking' && (
          <div className="flex items-center gap-2 text-[13px] text-app-muted">
            <Spinner size={14} /> {t('settings.updates.checking')}
          </div>
        )}
        {status === 'available' && updateInfo && (
          <div className="space-y-3">
            <Banner tone="info" title={t('settings.updates.available', { version: updateInfo.version })}>
              {updateInfo.body}
            </Banner>
            <div className="flex gap-2">
              <Button onClick={downloadAndInstall} leftIcon={<Download className="size-3.5" />}>
                {t('settings.updates.downloadInstall')}
              </Button>
              <Button variant="secondary" onClick={dismiss}>{t('common.later')}</Button>
            </div>
          </div>
        )}
        {status === 'downloading' && (
          <div className="space-y-2">
            <div className="flex items-center gap-2 text-[13px] text-app-muted">
              <Spinner size={14} /> {t('settings.updates.downloading', { progress: Math.round(progress) })}
            </div>
            <div className="w-full h-1.5 rounded-full bg-app-raised overflow-hidden shadow-[inset_0_1px_2px_rgba(0,0,0,0.15)]">
              <div className="h-full bg-app-accent rounded-full transition-[width] duration-300 ease-app-out" style={{ width: `${progress}%` }} />
            </div>
          </div>
        )}
        {status === 'ready' && (
          <div className="space-y-3">
            <p className="text-[13px] text-app-success">{t('settings.updates.ready')}</p>
            <Button onClick={restartApp}>{t('common.restartNow')}</Button>
          </div>
        )}
        {status === 'error' && (
          <div className="space-y-2">
            <p className="text-[13px] text-app-danger">{error || t('settings.updates.errorDefault')}</p>
            <Button variant="ghost" onClick={checkForUpdates}>{t('common.tryAgain')}</Button>
          </div>
        )}
      </div>
    </SettingsSection>
  );
}

/* ----------------------------------------------------------------------------
   Main Settings component
   ------------------------------------------------------------------------- */

export function Settings() {
  const { t } = useTranslation();
  const {
    aiPolishEnabled, telemetryEnabled, shortcut, handsFreeMode, hidePillWhenInactive,
    autostartEnabled, historyEnabled, vadAutoStopEnabled, vadSilenceSecs, audioDeviceName,
    transcriptionLanguage, diagnosticsEnabled, language, theme, dictionary, history, loading, isPro, licenseKey,
    licenseStatus, licenseExpiresAt, licenseActivationCount, licenseActivationLimit,
    licenseLoading, licenseError,
    loadSettings, saveSettings, resetSettings, loadDictionary, deleteEntry,
    clearDictionary, loadHistory, clearHistory, loadLicense, activateLicense,
    deactivateLicense, validateLicense, loadUsage,
  } = useSettingsStore();

  const isMac = navigator.platform.startsWith('Mac');
  const [appVersion, setAppVersion] = useState('...');
  const updateSectionRef = useRef<HTMLDivElement>(null);

  useEffect(() => { getVersion().then(setAppVersion).catch(() => {}); }, []);

  // Keep window title in sync with the active language. Deps include `t` so
  // the effect re-fires only on language change, not every render.
  useEffect(() => {
    try { getCurrentWindow().setTitle(t('windowTitle.settings')); }
    catch { /* not in Tauri (dev preview) */ }
  }, [t]);

  useTauriEvent<{ version: string; body?: string }>('update-available', () => {
    updateSectionRef.current?.scrollIntoView({ behavior: 'smooth', block: 'center' });
  });

  const [showClearConfirm, setShowClearConfirm] = useState(false);
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [showClearHistoryConfirm, setShowClearHistoryConfirm] = useState(false);
  const [shortcutError, setShortcutError] = useState('');
  const [shortcutSuccess, setShortcutSuccess] = useState(false);
  const [groqApiKey, setGroqApiKey] = useState('');
  const [hasGroqKey, setHasGroqKey] = useState(false);
  const [groqKeySaving, setGroqKeySaving] = useState(false);
  const [groqKeySuccess, setGroqKeySuccess] = useState(false);
  const [groqKeyError, setGroqKeyError] = useState('');
  const [newOriginal, setNewOriginal] = useState('');
  const [newCorrection, setNewCorrection] = useState('');
  const [addEntryError, setAddEntryError] = useState('');
  const [showRestartBanner, setShowRestartBanner] = useState(false);
  const [licenseInput, setLicenseInput] = useState('');
  const [showDeactivateConfirm, setShowDeactivateConfirm] = useState(false);
  const [showUninstallConfirm, setShowUninstallConfirm] = useState(false);
  const [uninstalling, setUninstalling] = useState(false);

  // No local autostart state and no `isAutostartEnabled()` call — the plugin's
  // is_enabled() is unreliable on macOS for product names with spaces. The UI
  // reads `autostartEnabled` from the store (settings.json); the LaunchAgent
  // plist is the OS side-effect that the toggle keeps in sync.

  // Proactive Input Monitoring check when the saved shortcut is FnKey.
  useEffect(() => {
    if (shortcut !== 'FnKey') {
      if (shortcutError.includes('Input Monitoring') || shortcutError === 'error.input_monitoring_required') {
        setShortcutError('');
      }
      return;
    }
    invoke<boolean>('check_input_monitoring')
      .then((hasPermission) => {
        if (!hasPermission) setShortcutError('error.input_monitoring_required');
        else if (shortcutError.includes('Input Monitoring') || shortcutError === 'error.input_monitoring_required') {
          setShortcutError('');
        }
      })
      .catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shortcut]);

  const handleAutostartToggle = async (enabled: boolean) => {
    try {
      // Real OS effect: register/unregister LaunchAgent plist (mac) /
      // registry key (Windows).
      if (enabled) await enableAutostart(); else await disableAutostart();
      // Persist the user's intent to settings.json so the UI doesn't depend
      // on the plugin's flaky is_enabled() readback.
      await saveSettings({ autostart_enabled: enabled });
      trackEvent('setting_changed', { setting_name: 'autostart_enabled', new_value: String(enabled) });
    } catch (error) { console.error('Failed to update autostart:', error); }
  };

  const checkApiKeys = useCallback(() => {
    invoke<boolean>('has_groq_api_key').then(setHasGroqKey).catch(console.error);
  }, []);

  useEffect(() => {
    loadSettings(); loadDictionary(); loadHistory(); loadLicense(); loadUsage(); checkApiKeys();
  }, [loadSettings, loadDictionary, loadHistory, loadLicense, loadUsage, checkApiKeys]);

  useTauriEvent('license-changed', () => { loadLicense(); loadUsage(); });

  useTauriEvent<string>('recording-state-changed', (event) => {
    if (event.payload === 'Idle') {
      setTimeout(() => loadHistory(), 500);
      setTimeout(() => loadUsage(), 600);
    }
  });

  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) checkApiKeys();
    });
    return () => { unlisten.then((fn) => fn()); };
  }, [checkApiKeys]);

  useTauriEvent('dictionary-changed', () => { loadDictionary(); loadUsage(); });

  /* ---- Generic toggle factory: cuts 7 near-identical handlers down to 1 --- */
  const makeToggle = useCallback(
    <K extends 'ai_polish_enabled' | 'telemetry_enabled' | 'hands_free_mode' | 'hide_pill_when_inactive' | 'history_enabled' | 'vad_auto_stop_enabled' | 'diagnostics_enabled'>(
      key: K,
      sideEffect?: () => void,
    ) => async (enabled: boolean) => {
      try {
        await saveSettings({ [key]: enabled } as Partial<Record<K, boolean>>);
        trackEvent('setting_changed', { setting_name: key, new_value: String(enabled) });
        sideEffect?.();
      } catch (error) { console.error(`Failed to save ${key}:`, error); }
    },
    [saveSettings],
  );

  const handlePolishToggle = makeToggle('ai_polish_enabled');
  const handleTelemetryToggle = makeToggle('telemetry_enabled', () => setShowRestartBanner(true));
  const handleHandsFreeModeToggle = makeToggle('hands_free_mode');
  const handleHidePillWhenInactiveToggle = makeToggle('hide_pill_when_inactive');
  const handleHistoryEnabledToggle = makeToggle('history_enabled');
  const handleVadAutoStopToggle = makeToggle('vad_auto_stop_enabled');
  const handleDiagnosticsToggle = makeToggle('diagnostics_enabled');
  const handleVadSilenceSecsChange = useCallback(
    async (raw: number) => {
      // Clamp to the same window the Rust side enforces.
      const next = Math.min(10, Math.max(1, Math.round(raw)));
      try {
        await saveSettings({ vad_silence_secs: next });
      } catch (error) {
        console.error('Failed to save vad_silence_secs:', error);
      }
    },
    [saveSettings],
  );

  // Audio input devices: enumerated on Settings open + on focus return so
  // a user who hot-plugs a USB mic sees it without restarting.
  const [audioDevices, setAudioDevices] = useState<Array<{ name: string; is_default: boolean }>>([]);
  const refreshAudioDevices = useCallback(() => {
    invoke<Array<{ name: string; is_default: boolean }>>('list_audio_input_devices')
      .then(setAudioDevices)
      .catch((e) => console.error('[Settings] list_audio_input_devices failed:', e));
  }, []);
  useEffect(() => {
    refreshAudioDevices();
  }, [refreshAudioDevices]);
  const handleAudioDeviceChange = useCallback(
    async (raw: string) => {
      // Empty string from the <select> means "use the OS default".
      const next = raw === '' ? null : raw;
      try {
        await saveSettings({ audio_device_name: next });
      } catch (error) {
        console.error('Failed to save audio_device_name:', error);
      }
    },
    [saveSettings],
  );

  const handleGroqKeySave = async () => {
    if (!groqApiKey.trim()) return;
    setGroqKeySaving(true); setGroqKeyError(''); setGroqKeySuccess(false);
    try {
      await invoke('validate_groq_api_key', { key: groqApiKey });
      await invoke('set_groq_api_key', { key: groqApiKey });
      setHasGroqKey(true); setGroqApiKey(''); setGroqKeySuccess(true);
      setTimeout(() => setGroqKeySuccess(false), 3000);
    } catch (error) { setGroqKeyError(String(error)); }
    finally { setGroqKeySaving(false); }
  };

  const handleShortcutChange = async (newShortcut: string) => {
    setShortcutError(''); setShortcutSuccess(false);
    try {
      const isFnKey = newShortcut === 'FnKey';
      if (isFnKey) {
        const hasPermission = await invoke<boolean>('check_input_monitoring');
        if (!hasPermission) { setShortcutError('error.input_monitoring_required'); return; }
        try { await invoke('unregister_shortcuts_cmd'); } catch {}
        await invoke('set_fn_key_enabled', { enabled: true });
        await saveSettings({ shortcut: 'FnKey', fn_key_enabled: true });
        trackEvent('setting_changed', { setting_name: 'shortcut', new_value: 'FnKey' });
      } else {
        await invoke('set_fn_key_enabled', { enabled: false });
        await invoke('update_shortcut_cmd', { shortcut: newShortcut });
        await saveSettings({ shortcut: newShortcut, fn_key_enabled: false });
        trackEvent('setting_changed', { setting_name: 'shortcut', new_value: newShortcut });
      }
      setShortcutSuccess(true);
      setTimeout(() => setShortcutSuccess(false), 3000);
    } catch (error) {
      console.error('Failed to update shortcut:', error);
      setShortcutError(String(error));
    }
  };

  const handleClearDictionary = async () => {
    try { await clearDictionary(); setShowClearConfirm(false); }
    catch (error) { console.error('Failed to clear dictionary:', error); }
  };

  const handleResetDefaults = async () => {
    try { await resetSettings(); await clearDictionary(); setShowResetConfirm(false); }
    catch (error) { console.error('Failed to reset settings:', error); }
  };

  const handleAddEntry = async () => {
    setAddEntryError('');
    const orig = newOriginal.trim(); const corr = newCorrection.trim();
    if (!orig || !corr) { setAddEntryError(t('settings.dictionary.bothRequired')); return; }
    if (orig === corr) { setAddEntryError(t('settings.dictionary.mustBeDifferent')); return; }
    try {
      await invoke('add_dictionary_entry', { original: orig, correction: corr });
      setNewOriginal(''); setNewCorrection('');
      await loadDictionary(); await loadUsage();
    } catch (error) { setAddEntryError(String(error)); }
  };

  const handleDeleteEntry = useCallback(async (original: string) => {
    try { await deleteEntry(original); }
    catch (error) { console.error('Failed to delete entry:', error); }
  }, [deleteEntry]);

  const handleClearHistory = async () => {
    try { await clearHistory(); setShowClearHistoryConfirm(false); }
    catch (error) { console.error('Failed to clear history:', error); }
  };

  const handleActivateLicense = async () => {
    const key = licenseInput.trim();
    if (!key) return;
    try {
      await activateLicense(key); setLicenseInput('');
      trackEvent('license_activated', {});
    } catch (error) { console.error('Activation failed:', error); }
  };

  const handleDeactivateLicense = async () => {
    try {
      await deactivateLicense(); setShowDeactivateConfirm(false);
      trackEvent('license_deactivated', {});
    } catch (error) { console.error('Deactivation failed:', error); }
  };

  const handleUninstall = async () => {
    setUninstalling(true);
    try {
      // Rust will wipe keychain, spawn the detached uninstaller script,
      // and exit the process. Anything below this line on a successful
      // path won't run.
      await invoke('uninstall_app');
    } catch (error) {
      console.error('Uninstall failed:', error);
      setUninstalling(false);
      setShowUninstallConfirm(false);
    }
  };

  const formatExpiry = (ts: number | null): string => {
    if (!ts) return t('settings.pro.neverExpires');
    return t('settings.pro.expiresOn', { date: new Date(ts * 1000).toLocaleDateString() });
  };

  const maskedLicenseKey = licenseKey ? `${licenseKey.slice(0, 4)}…${licenseKey.slice(-4)}` : '';

  // No caps, so no "at cap" states, no trial countdown, and no x/y rows
  // counting down to a paywall. Every feature is free and unlimited; a
  // licence is a thank-you, not a key.

  const triggerOptions = isMac
    ? [
        { value: 'FnKey', label: t('settings.recordingTrigger.optionFn'), desc: t('settings.recordingTrigger.descRecommended'), recommended: true },
        { value: 'Alt+Space', label: t('settings.recordingTrigger.optionAltSpace'), desc: t('settings.recordingTrigger.descAltSpace'), recommended: false },
        { value: 'CmdOrCtrl+Shift+R', label: t('settings.recordingTrigger.optionCmdShiftR'), desc: t('settings.recordingTrigger.descCmdShiftR'), recommended: false },
      ]
    : [
        { value: 'Ctrl+Space', label: t('settings.recordingTrigger.optionCtrlSpace'), desc: t('settings.recordingTrigger.descRecommended'), recommended: true },
        { value: 'Ctrl+Shift+Space', label: t('settings.recordingTrigger.optionCtrlShiftSpace'), desc: '', recommended: false },
        { value: 'Super+J', label: t('settings.recordingTrigger.optionWinJ'), desc: t('settings.recordingTrigger.descNoConflicts'), recommended: false },
      ];

  const translateIfKey = (s: string | null): string =>
    s && (s.startsWith('error.') || s.startsWith('permission.')) ? t(s) : (s ?? '');
  const showShortcutError = translateIfKey(shortcutError);
  const showLicenseError = translateIfKey(licenseError);
  const shortcutErrorIsInputMonitoring =
    shortcutError.includes('Input Monitoring') || shortcutError === 'error.input_monitoring_required';

  const languageOptions: { value: LanguageChoice; label: string }[] = [
    { value: 'system', label: t('settings.language.optionSystem') },
    { value: 'en', label: t('settings.language.optionEnglish') },
    { value: 'fr', label: t('settings.language.optionFrench') },
  ];

  const themeOptions: { value: ThemeChoice; label: string }[] = [
    { value: 'system', label: t('settings.theme.optionSystem') },
    { value: 'light', label: t('settings.theme.optionLight') },
    { value: 'dark', label: t('settings.theme.optionDark') },
  ];

  return (
    <div className="h-screen flex bg-app-bg text-app-text bg-noise">
      <SettingsSidebar />
      <main className="flex-1 min-w-0 overflow-y-auto">
        <div className="max-w-2xl mx-auto px-8 pt-8 pb-12">
          <PermissionBanner />

          {/* ===== GENERAL ===== */}
          <div id="general" data-section="general" className="scroll-mt-6">
            <SettingsSection bare>
              <div className="flex items-start gap-4 mb-4">
                <BrandTile size="md" />
                <div className="min-w-0">
                  <h1 className="text-display-sm text-app-text">TTP by AmirKS</h1>
                  <p className="text-[12px] text-app-accent font-medium">{t('settings.about.subtitle', { version: appVersion })}</p>
                </div>
              </div>
              <p className="text-[13px] text-app-muted leading-relaxed">{t('settings.about.description')}</p>
            </SettingsSection>

            <SettingsSection title={t('settings.language.title')} description={t('settings.language.desc')}>
              <div className="space-y-2">
                {languageOptions.map((opt) => (
                  <RadioOption
                    key={opt.value}
                    selected={language === opt.value}
                    onSelect={async () => {
                      if (language === opt.value) return;
                      try {
                        await saveSettings({ language: opt.value });
                        trackEvent('setting_changed', { setting_name: 'language', new_value: opt.value });
                      } catch (error) { console.error('Failed to save language setting:', error); }
                    }}
                    label={opt.label}
                    disabled={loading}
                  />
                ))}
              </div>
            </SettingsSection>

            <SettingsSection title={t('settings.theme.title')} description={t('settings.theme.desc')}>
              <div className="space-y-2">
                {themeOptions.map((opt) => (
                  <RadioOption
                    key={opt.value}
                    selected={theme === opt.value}
                    onSelect={async () => {
                      if (theme === opt.value) return;
                      try {
                        await saveSettings({ theme: opt.value });
                        trackEvent('setting_changed', { setting_name: 'theme', new_value: opt.value });
                      } catch (error) { console.error('Failed to save theme setting:', error); }
                    }}
                    label={opt.label}
                    disabled={loading}
                  />
                ))}
              </div>
            </SettingsSection>

            <SettingsSection title={t('settings.startup.title')}>
              <SettingsRow
                label={t('settings.recordingMode.launchStartupLabel')}
                description={t('settings.recordingMode.launchStartupDesc')}
                control={<Toggle enabled={autostartEnabled} onChange={handleAutostartToggle} disabled={loading} />}
              />
            </SettingsSection>

            <div ref={updateSectionRef}>
              <UpdatesCard />
            </div>

            <SettingsSection title={t('settings.privacy.title')}>
              {showRestartBanner && (
                <Banner
                  tone="warning"
                  className="mb-4"
                  action={
                    <Button variant="secondary" size="sm" onClick={() => relaunch().catch(console.error)}>
                      {t('common.restartNow')}
                    </Button>
                  }
                >
                  {t('settings.privacy.restartHint')}
                </Banner>
              )}
              <SettingsRow
                label={t('settings.privacy.helpLabel')}
                description={t('settings.privacy.helpDesc')}
                control={<Toggle enabled={telemetryEnabled} onChange={handleTelemetryToggle} disabled={loading} />}
              />
              <div className="mt-4 p-3 bg-app-raised rounded-app-sm border border-app-border">
                <p className="text-[12px] text-app-muted leading-relaxed">
                  <span className="font-medium text-app-text">{t('settings.privacy.whatSentLabel')}</span>{' '}
                  {t('settings.privacy.whatSentBody')}
                </p>
                <p className="text-[12px] text-app-muted leading-relaxed mt-2">
                  <span className="font-medium text-app-text">{t('settings.privacy.neverSentLabel')}</span>{' '}
                  {t('settings.privacy.neverSentBody')}
                </p>
              </div>
            </SettingsSection>
          </div>

          {/* ===== CAPTURE ===== */}
          <div id="capture" data-section="capture" className="scroll-mt-6">
            <SettingsSection title={t('settings.recordingTrigger.title')} description={t('settings.recordingTrigger.desc')}>
              <div className="space-y-2">
                {triggerOptions.map((opt) => (
                  <RadioOption
                    key={opt.value}
                    selected={shortcut === opt.value}
                    onSelect={() => handleShortcutChange(opt.value)}
                    label={<span className="font-mono">{opt.label}</span>}
                    trailing={opt.desc ? <span className={opt.recommended ? 'text-app-accent font-medium' : undefined}>{opt.desc}</span> : undefined}
                    disabled={loading}
                  />
                ))}
              </div>

              <FnEmojiNudge />

              {shortcutError && (
                <div className="mt-4 space-y-2">
                  <p className="text-[13px] text-app-danger">{showShortcutError}</p>
                  {shortcutErrorIsInputMonitoring && (
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => invoke('open_input_monitoring_settings').catch(console.error)}
                      leftIcon={<ExternalLink className="size-3" />}
                    >
                      {t('settings.recordingTrigger.openInputMonitoring')}
                    </Button>
                  )}
                </div>
              )}
              {shortcutSuccess && (
                <p className="mt-3 text-[13px] text-app-success">{t('settings.recordingTrigger.successUpdated')}</p>
              )}
            </SettingsSection>

            <SettingsSection title={t('settings.recordingMode.title')}>
              <SettingsRow
                label={t('settings.recordingMode.handsFreeLabel')}
                description={t('settings.recordingMode.handsFreeDesc')}
                control={<Toggle enabled={handsFreeMode} onChange={handleHandsFreeModeToggle} disabled={loading} />}
              />
              <div className="border-t border-app-border my-1" />
              <SettingsRow
                label={t('settings.recordingMode.hidePillLabel')}
                description={t('settings.recordingMode.hidePillDesc')}
                control={<Toggle enabled={hidePillWhenInactive} onChange={handleHidePillWhenInactiveToggle} disabled={loading} />}
              />
              <div className="border-t border-app-border my-1" />
              <SettingsRow
                label={t('settings.recordingMode.vadAutoStopLabel')}
                description={t('settings.recordingMode.vadAutoStopDesc')}
                control={<Toggle enabled={vadAutoStopEnabled} onChange={handleVadAutoStopToggle} disabled={loading} />}
              />
              {vadAutoStopEnabled && (
                <div className="pl-1 py-2 flex items-center gap-3">
                  <label
                    htmlFor="vad-silence-secs"
                    className="text-[12px] text-app-muted shrink-0"
                  >
                    {t('settings.recordingMode.vadSilenceSecsLabel')}
                  </label>
                  <input
                    id="vad-silence-secs"
                    type="range"
                    min={1}
                    max={10}
                    step={1}
                    value={vadSilenceSecs}
                    onChange={(e) => handleVadSilenceSecsChange(Number(e.target.value))}
                    disabled={loading}
                    className="flex-1 accent-app-accent"
                  />
                  <span className="text-[12px] font-medium tabular-nums text-app-text w-10 text-right">
                    {vadSilenceSecs}s
                  </span>
                </div>
              )}
              <div className="border-t border-app-border my-1" />
              <div className="py-2">
                <div className="flex items-start justify-between gap-4">
                  <div className="min-w-0">
                    <label
                      htmlFor="audio-device-select"
                      className="text-[13px] font-medium text-app-text"
                    >
                      {t('settings.recordingMode.audioDeviceLabel')}
                    </label>
                    <p className="mt-0.5 text-[12px] text-app-muted leading-relaxed">
                      {t('settings.recordingMode.audioDeviceDesc')}
                    </p>
                  </div>
                  <select
                    id="audio-device-select"
                    value={audioDeviceName ?? ''}
                    onChange={(e) => handleAudioDeviceChange(e.target.value)}
                    onFocus={refreshAudioDevices}
                    disabled={loading}
                    className="h-8 rounded-app-md border border-app-border bg-app-surface px-2 text-[13px] text-app-text shrink-0 max-w-[55%] focus:border-app-accent focus:outline-none"
                  >
                    <option value="">{t('settings.recordingMode.audioDeviceDefault')}</option>
                    {audioDevices.map((d) => (
                      <option key={d.name} value={d.name}>
                        {d.is_default ? `${d.name} ${t('settings.recordingMode.audioDeviceDefaultSuffix')}` : d.name}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
            </SettingsSection>

            <SettingsSection title={t('settings.transcription.title')}>
              <div className="space-y-3 mb-5">
                <p className="text-[13px] font-medium text-app-text">{t('settings.transcription.groqLabel')}</p>
                {hasGroqKey ? (
                  <div className="flex items-center gap-3">
                    <span className="text-[13px] text-app-success">{t('settings.transcription.keyConfigured')}</span>
                    <Button variant="ghost" size="sm" onClick={() => setHasGroqKey(false)}>
                      {t('common.change')}
                    </Button>
                  </div>
                ) : (
                  <>
                    <div className="flex gap-2">
                      <Input
                        type="password"
                        value={groqApiKey}
                        onChange={(e) => setGroqApiKey(e.target.value)}
                        placeholder={t('settings.transcription.keyPlaceholder')}
                        className="flex-1"
                        aria-label={t('form.apiKey.label')}
                        autoComplete="off"
                      />
                      <Button
                        onClick={handleGroqKeySave}
                        disabled={groqKeySaving || !groqApiKey.trim()}
                        loading={groqKeySaving}
                      >
                        {groqKeySaving ? t('common.validating') : t('common.save')}
                      </Button>
                    </div>
                    {groqKeySuccess && <p className="text-[13px] text-app-success">{t('settings.transcription.keySaved')}</p>}
                    {groqKeyError && <p className="text-[13px] text-app-danger">{groqKeyError}</p>}
                    <p className="text-[12px] text-app-muted">
                      {t('settings.transcription.getKeyAt')}{' '}
                      <a href="https://console.groq.com/keys" target="_blank" rel="noopener noreferrer" className="text-app-accent hover:underline">
                        console.groq.com
                      </a>
                    </p>
                  </>
                )}
              </div>
              <div className="border-t border-app-border pt-4">
                <SettingsRow
                  label={t('settings.transcription.polishLabel')}
                  description={t('settings.transcription.polishDesc')}
                  control={<Toggle enabled={aiPolishEnabled} onChange={handlePolishToggle} disabled={loading} />}
                />
              </div>
              <div className="border-t border-app-border pt-4 py-2">
                <div className="flex items-start justify-between gap-4">
                  <div className="min-w-0">
                    <label
                      htmlFor="transcription-language-select"
                      className="text-[13px] font-medium text-app-text"
                    >
                      {t('settings.transcription.languageLabel')}
                    </label>
                    <p className="mt-0.5 text-[12px] text-app-muted leading-relaxed">
                      {t('settings.transcription.languageDesc')}
                    </p>
                  </div>
                  <select
                    id="transcription-language-select"
                    value={transcriptionLanguage}
                    onChange={(e) =>
                      saveSettings({
                        transcription_language: e.target.value === 'auto' ? null : e.target.value,
                      }).catch((err) =>
                        console.error('Failed to save transcription_language:', err),
                      )
                    }
                    disabled={loading}
                    className="h-8 rounded-app-md border border-app-border bg-app-surface px-2 text-[13px] text-app-text shrink-0 focus:border-app-accent focus:outline-none"
                  >
                    <option value="auto">{t('settings.transcription.languageAuto')}</option>
                    <option value="en">{t('settings.transcription.languageEnglish')}</option>
                    <option value="fr">{t('settings.transcription.languageFrench')}</option>
                  </select>
                </div>
              </div>
            </SettingsSection>
          </div>

          {/* ===== PRO =====
              Analytics moved to the sidebar footer (SidebarStats) — keeps
              "your numbers" visible at all times without making the user
              scroll past it. */}
          <div id="pro" data-section="pro" className="scroll-mt-6">
            <SettingsSection
              title={t('settings.pro.title')}
              action={
                <div className="flex items-center gap-2">
                  <Crown className={cn('size-4', isPro ? 'text-app-warning' : 'text-app-faint')} />
                  {isPro && (
                    <span className="px-2 py-0.5 text-[11px] font-semibold rounded-full bg-app-warning-tint text-app-warning">
                      {t('settings.pro.badgeActive')}
                    </span>
                  )}
                </div>
              }
            >
              {!isPro ? (
                <>
                  <p className="text-[13px] text-app-muted mb-4">
                    {t('settings.pro.descFree')}
                  </p>
                  <div className="space-y-2 mb-4">
                    <Input
                      type="text"
                      value={licenseInput}
                      onChange={(e) => setLicenseInput(e.target.value)}
                      placeholder={t('settings.pro.inputPlaceholder')}
                      spellCheck={false}
                      disabled={licenseLoading}
                      className="font-mono"
                      onKeyDown={(e) => { if (e.key === 'Enter' && licenseInput.trim()) handleActivateLicense(); }}
                      aria-label={t('settings.pro.labelKey')}
                    />
                    {licenseError && <p className="text-[12px] text-app-danger">{showLicenseError}</p>}
                  </div>
                  <div className="flex items-center gap-3">
                    <Button onClick={handleActivateLicense} disabled={licenseLoading || !licenseInput.trim()} loading={licenseLoading}>
                      {t('settings.pro.activate')}
                    </Button>
                    <a
                      href="https://amirks.lemonsqueezy.com/checkout/buy/23ded1c4-c862-4f8c-ada5-0bb3dc2e0060"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="inline-flex items-center gap-1 text-[13px] font-medium text-app-accent hover:text-app-accent-hover"
                    >
                      {t('settings.pro.buyLink')}
                      <ArrowRight className="size-3.5" />
                    </a>
                  </div>
                </>
              ) : (
                <>
                  <div className="space-y-2 mb-4">
                    <ProInfoRow label={t('settings.pro.labelKey')} value={<span className="font-mono">{maskedLicenseKey}</span>} />
                    <ProInfoRow label={t('settings.pro.labelStatus')} value={<span className="capitalize">{licenseStatus ?? t('settings.pro.statusUnknown')}</span>} />
                    <ProInfoRow label={t('settings.pro.labelValidity')} value={formatExpiry(licenseExpiresAt)} />
                    {licenseActivationLimit !== null && (
                      <ProInfoRow label={t('settings.pro.labelActivations')} value={`${licenseActivationCount ?? 0} / ${licenseActivationLimit}`} />
                    )}
                  </div>
                  {licenseError && <p className="text-[12px] text-app-danger mb-3">{showLicenseError}</p>}
                  <div className="flex items-center gap-3">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => setShowDeactivateConfirm(true)}
                      disabled={licenseLoading}
                      className="text-app-danger hover:text-app-danger hover:bg-app-danger-tint"
                    >
                      {t('settings.pro.deactivateDevice')}
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={validateLicense}
                      disabled={licenseLoading}
                      leftIcon={licenseLoading ? <Spinner size={12} /> : <RefreshCw className="size-3" />}
                    >
                      {t('common.refresh')}
                    </Button>
                  </div>
                </>
              )}
            </SettingsSection>
          </div>

          {/* ===== DATA ===== */}
          <div id="data" data-section="data" className="scroll-mt-6">
            <SettingsSection
              title={t('settings.dictionary.title')}
              action={dictionary.length > 0 ? (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setShowClearConfirm(true)}
                  className="text-app-danger hover:text-app-danger hover:bg-app-danger-tint"
                >
                  {t('common.clearAll')}
                </Button>
              ) : undefined}
            >
              <div className="mb-4 flex gap-2 items-end">
                <div className="flex-1">
                  <label htmlFor="dict-original" className="block text-[11px] text-app-muted mb-1 font-medium">{t('settings.dictionary.labelMisheard')}</label>
                  <Input
                    id="dict-original"
                    type="text"
                    value={newOriginal}
                    onChange={(e) => setNewOriginal(e.target.value)}
                    placeholder={t('settings.dictionary.placeholderMisheard')}
                  />
                </div>
                <ArrowRight className="size-4 text-app-faint shrink-0 mb-2.5" aria-hidden />
                <div className="flex-1">
                  <label htmlFor="dict-correction" className="block text-[11px] text-app-muted mb-1 font-medium">{t('settings.dictionary.labelCorrection')}</label>
                  <Input
                    id="dict-correction"
                    type="text"
                    value={newCorrection}
                    onChange={(e) => setNewCorrection(e.target.value)}
                    placeholder={t('settings.dictionary.placeholderCorrection')}
                  />
                </div>
                <Button onClick={handleAddEntry} disabled={!newOriginal.trim() || !newCorrection.trim()}>
                  {t('common.add')}
                </Button>
              </div>
              {addEntryError && <p className="text-[13px] text-app-danger mb-3">{addEntryError}</p>}

              {dictionary.length === 0 ? (
                <EmptyState
                  icon={<BookOpen />}
                  title={t('settings.dictionary.emptyTitle')}
                  description={t('settings.dictionary.emptyState')}
                />
              ) : (
                <div className="rounded-app-sm border border-app-border overflow-hidden">
                  <div className="bg-app-raised border-b border-app-border px-4 py-2 grid grid-cols-2 gap-3">
                    <span className="text-[10px] font-semibold uppercase tracking-[0.06em] text-app-faint">{t('settings.dictionary.tableOriginal')}</span>
                    <span className="text-[10px] font-semibold uppercase tracking-[0.06em] text-app-faint">{t('settings.dictionary.tableCorrection')}</span>
                  </div>
                  <div className="divide-y divide-app-border">
                    {dictionary.map((entry) => (
                      <DictionaryRow key={entry.original} entry={entry} onDelete={handleDeleteEntry} />
                    ))}
                  </div>
                </div>
              )}
            </SettingsSection>

            <SettingsSection
              title={t('settings.history.title')}
              action={history.length > 0 ? (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setShowClearHistoryConfirm(true)}
                  className="text-app-danger hover:text-app-danger hover:bg-app-danger-tint"
                >
                  {t('settings.history.clearButton')}
                </Button>
              ) : undefined}
            >
              <SettingsRow
                label={t('settings.history.saveLabel')}
                description={t('settings.history.saveDesc')}
                control={<Toggle enabled={historyEnabled} onChange={handleHistoryEnabledToggle} disabled={loading} />}
              />
              <div className="border-t border-app-border mt-3 pt-3">
                {history.length === 0 ? (
                  <EmptyState
                    icon={<Database />}
                    title={t('settings.history.emptyTitle')}
                    description={historyEnabled ? t('settings.history.emptyEnabled') : t('settings.history.emptyDisabled')}
                  />
                ) : (
                  <div className="max-h-80 overflow-y-auto rounded-app-sm border border-app-border divide-y divide-app-border">
                    {history.map((entry) => (
                      <HistoryRow key={entry.timestamp} entry={entry} />
                    ))}
                  </div>
                )}
              </div>
            </SettingsSection>
          </div>

          {/* ===== ADVANCED ===== */}
          <div id="advanced" data-section="advanced" className="scroll-mt-6">
            <UpdateChannelCard />

            <SettingsSection
              title={t('settings.logs.title')}
              description={t('settings.logs.desc')}
            >
              <SettingsRow
                label={t('settings.diagnostics.label')}
                description={t('settings.diagnostics.desc')}
                control={<Toggle enabled={diagnosticsEnabled} onChange={handleDiagnosticsToggle} disabled={loading} />}
              />
              <div className="mt-4 flex flex-wrap gap-2">
                <Button
                  variant="secondary"
                  onClick={() => {
                    invoke('reveal_log_folder').catch((e) =>
                      console.error('[Settings] reveal_log_folder failed:', e),
                    );
                  }}
                >
                  {t('settings.logs.button')}
                </Button>
                <Button
                  variant="secondary"
                  onClick={() => {
                    invoke('reveal_recordings_folder').catch((e) =>
                      console.error('[Settings] reveal_recordings_folder failed:', e),
                    );
                  }}
                >
                  {t('settings.logs.recordingsButton')}
                </Button>
              </div>
            </SettingsSection>

            <SettingsSection title={t('settings.reset.title')} description={t('settings.reset.desc')}>
              <Button
                variant="ghost"
                onClick={() => setShowResetConfirm(true)}
                className="text-app-danger hover:text-app-danger hover:bg-app-danger-tint border border-app-danger/30"
              >
                {t('settings.reset.button')}
              </Button>
            </SettingsSection>

            {/* Danger zone — full uninstall. Bottom of Advanced so users
                have to scroll past everything else to reach it. */}
            <SettingsSection
              title={t('settings.uninstall.title')}
              description={t('settings.uninstall.desc')}
            >
              <Button
                variant="danger"
                onClick={() => setShowUninstallConfirm(true)}
              >
                {t('settings.uninstall.button')}
              </Button>
            </SettingsSection>
          </div>

          <ConfirmDialog
            open={showClearConfirm}
            title={t('dialog.clearDictionary.title')}
            message={t('dialog.clearDictionary.message')}
            confirmText={t('dialog.clearDictionary.confirm')}
            cancelText={t('common.cancel')}
            onConfirm={handleClearDictionary}
            onCancel={() => setShowClearConfirm(false)}
          />
          <ConfirmDialog
            open={showResetConfirm}
            title={t('dialog.reset.title')}
            message={t('dialog.reset.message')}
            confirmText={t('dialog.reset.confirm')}
            cancelText={t('common.cancel')}
            onConfirm={handleResetDefaults}
            onCancel={() => setShowResetConfirm(false)}
          />
          <ConfirmDialog
            open={showClearHistoryConfirm}
            title={t('dialog.clearHistory.title')}
            message={t('dialog.clearHistory.message')}
            confirmText={t('dialog.clearHistory.confirm')}
            cancelText={t('common.cancel')}
            onConfirm={handleClearHistory}
            onCancel={() => setShowClearHistoryConfirm(false)}
          />
          <ConfirmDialog
            open={showUninstallConfirm}
            title={t('dialog.uninstall.title')}
            message={t('dialog.uninstall.message')}
            confirmText={uninstalling ? t('dialog.uninstall.uninstalling') : t('dialog.uninstall.confirm')}
            cancelText={t('common.cancel')}
            tone="danger"
            onConfirm={handleUninstall}
            onCancel={() => !uninstalling && setShowUninstallConfirm(false)}
          />
          <ConfirmDialog
            open={showDeactivateConfirm}
            title={t('dialog.deactivateLicense.title')}
            message={t('dialog.deactivateLicense.message')}
            confirmText={t('dialog.deactivateLicense.confirm')}
            cancelText={t('common.cancel')}
            tone="warning"
            onConfirm={handleDeactivateLicense}
            onCancel={() => setShowDeactivateConfirm(false)}
          />

          <WhatsNew />
        </div>
      </main>
    </div>
  );
}

/* ----------------------------------------------------------------------------
   Misc inline helpers
   ------------------------------------------------------------------------- */

function ProInfoRow({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between text-[13px]">
      <span className="text-app-muted">{label}</span>
      <span className="text-app-text">{value}</span>
    </div>
  );
}

/* ----------------------------------------------------------------------------
   SettingsSidebar — five IA groups. Active tracking via IntersectionObserver.
   ------------------------------------------------------------------------- */

interface SidebarItem { id: string; labelKey: string; icon: typeof Crown; }

function SettingsSidebar() {
  const { t } = useTranslation();
  const [appVersion, setAppVersion] = useState('');
  const [active, setActive] = useState('general');

  useEffect(() => { getVersion().then(setAppVersion).catch(() => {}); }, []);

  useEffect(() => {
    const targets = document.querySelectorAll('[data-section]');
    if (targets.length === 0) return;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            const id = (entry.target as HTMLElement).dataset.section;
            if (id) setActive(id);
            break;
          }
        }
      },
      { rootMargin: '-20% 0px -60% 0px', threshold: 0.01 },
    );
    targets.forEach((el) => observer.observe(el));
    return () => observer.disconnect();
  }, []);

  const items: SidebarItem[] = [
    { id: 'general', labelKey: 'settings.nav.general', icon: User },
    { id: 'capture', labelKey: 'settings.nav.capture', icon: Mic },
    { id: 'pro', labelKey: 'settings.nav.pro', icon: Crown },
    { id: 'data', labelKey: 'settings.nav.data', icon: Database },
    { id: 'advanced', labelKey: 'settings.nav.advanced', icon: SlidersHorizontal },
  ];

  const onSelect = (id: string) => {
    const el = document.getElementById(id);
    if (el) { el.scrollIntoView({ behavior: 'smooth', block: 'start' }); setActive(id); }
  };

  return (
    <aside className="w-56 shrink-0 bg-app-dim border-r border-app-border flex flex-col h-screen sticky top-0">
      <div className="px-5 pt-6 pb-4">
        <div className="flex items-center gap-2.5">
          <BrandTile size="sm" />
          <div className="min-w-0">
            <div className="text-[12px] font-semibold text-app-text leading-tight">TTP by AmirKS</div>
            <div className="text-[10px] text-app-faint tabular-nums leading-tight">v{appVersion || '…'}</div>
          </div>
        </div>
      </div>

      <nav className="flex-1 overflow-y-auto px-2 py-2">
        <ul className="space-y-px">
          {items.map((it) => {
            const Icon = it.icon;
            const isActive = active === it.id;
            return (
              <li key={it.id}>
                <button
                  type="button"
                  onClick={() => onSelect(it.id)}
                  className={cn(
                    'w-full flex items-center gap-2.5 px-2.5 py-1.5 rounded-app-sm text-[12px] font-medium',
                    'transition-colors duration-hover ease-app-out',
                    isActive
                      ? 'bg-app-surface text-app-text shine-sm'
                      : 'text-app-muted hover:text-app-text hover:bg-app-surface',
                  )}
                >
                  <Icon
                    className={cn('size-3.5 shrink-0', isActive ? 'text-app-accent' : 'text-app-faint')}
                    strokeWidth={1.75}
                    aria-hidden
                  />
                  <span className="truncate text-left">{t(it.labelKey)}</span>
                </button>
              </li>
            );
          })}
        </ul>
      </nav>

      <SidebarStats />

      <div className="border-t border-app-border px-4 py-3 bg-app-bg">
        <div className="flex items-center justify-between gap-2 text-[11px] text-app-faint">
          <a href="https://amirks.eu" target="_blank" rel="noopener noreferrer" className="hover:text-app-text transition-colors">
            amirks.eu
          </a>
          <a
            href="https://www.linkedin.com/in/amirks/"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1 hover:text-app-text transition-colors"
          >
            <span>{t('settings.sidebar.followLinkedIn')}</span>
            <ExternalLink className="size-2.5" aria-hidden />
          </a>
        </div>
      </div>
    </aside>
  );
}

/* ----------------------------------------------------------------------------
   SidebarStats — sleek mini display of this-month transcription activity.
   Lives in the sidebar so users see their numbers at all times without
   scrolling. Refreshes live when a transcription completes.
   ------------------------------------------------------------------------- */

function SidebarStats() {
  const { t, i18n } = useTranslation();
  const [summary, setSummary] = useState<AnalyticsSummary | null>(null);

  const load = useCallback(() => {
    invoke<AnalyticsSummary>('get_analytics_summary')
      .then(setSummary)
      .catch(() => {});
  }, []);

  useEffect(() => { load(); }, [load]);

  useTauriEvent<string>('recording-state-changed', (event) => {
    if (event.payload === 'Idle') setTimeout(load, 600);
  });

  if (!summary || summary.all_time.transcriptions === 0) return null;

  const fmt = new Intl.NumberFormat(i18n.language, { notation: 'compact', maximumFractionDigits: 1 }).format;

  return (
    <div className="border-t border-app-border px-4 py-3 bg-app-bg/50">
      <p className="text-[9px] font-semibold uppercase tracking-[0.08em] text-app-faint mb-1.5">
        {t('settings.sidebar.thisMonth')}
      </p>
      <div className="flex items-baseline gap-1.5">
        <span className="text-[15px] font-semibold text-app-text tabular-nums leading-none tracking-[-0.01em]">
          {fmt(summary.month.words)}
        </span>
        <span className="text-[10px] text-app-muted">{t('settings.sidebar.words')}</span>
      </div>
      <div className="mt-1 flex items-center gap-1.5 text-[10px] text-app-faint">
        <span className="size-1 rounded-full bg-app-accent" aria-hidden />
        <span className="tabular-nums">
          {t('settings.sidebar.transcriptions', { count: summary.month.transcriptions })}
        </span>
      </div>
    </div>
  );
}

export default Settings;
