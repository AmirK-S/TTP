// TTP - Talk To Paste
// Settings window, macOS System Settings style: a sidebar of short pages, and
// on each page a few groups of compact rows — label on the left, control on
// the right, a one-line hint only where the label is not enough.

import { useEffect, useState, useCallback, useRef, memo, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Volume2, Copy, Check, Download, RefreshCw, ArrowRight, Repeat, Trash2,
  Mic, SlidersHorizontal, BookOpen, History, Heart, Wrench,
} from 'lucide-react';
import { cn } from '../lib/cn';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useTauriEvent } from '../hooks/useTauriEvent';
import { getVersion } from '@tauri-apps/api/app';
import { relaunch } from '@tauri-apps/plugin-process';
import { enable as enableAutostart, disable as disableAutostart } from '@tauri-apps/plugin-autostart';
import { useUpdater } from '../hooks/useUpdater';
import { useSettingsStore, DictionaryEntry, HistoryEntry } from '../stores/settings-store';
import { PermissionBanner } from '../components/PermissionBanner';
import { FnEmojiNudge } from '../components/FnEmojiNudge';
import { ApiKeyForm } from '../components/ApiKeyForm';
import { TriggerPicker } from '../components/TriggerPicker';
import WhatsNew from '../components/WhatsNew';
import { Button, Input, Banner, Toggle, ConfirmDialog, BrandTile } from '../components/ui';

/* ----------------------------------------------------------------------------
   Layout primitives
   ------------------------------------------------------------------------- */

const SELECT = cn(
  'h-7 max-w-[230px] rounded-app-sm border border-app-border bg-app-bg px-2',
  'text-[12.5px] text-app-text focus:border-app-accent focus:outline-none',
);

/** A titled group of rows on one rounded surface. */
function Group({ title, action, children, className }: {
  title: string;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={cn('mb-6', className)}>
      <div className="mb-2 flex min-h-[22px] items-center justify-between px-1">
        <h2 className="text-[12.5px] font-semibold text-app-muted">{title}</h2>
        {action}
      </div>
      <div className="overflow-hidden rounded-app-md border border-app-border bg-app-surface divide-y divide-app-border">
        {children}
      </div>
    </section>
  );
}

/** One setting: label (and an optional one-line hint) left, control right. */
function Row({ label, hint, children, htmlFor }: {
  label: ReactNode;
  hint?: ReactNode;
  children?: ReactNode;
  htmlFor?: string;
}) {
  return (
    <div className="flex min-h-[46px] items-center justify-between gap-4 px-4 py-2.5">
      <div className="min-w-0">
        {htmlFor ? (
          <label htmlFor={htmlFor} className="block text-[13px] text-app-text">{label}</label>
        ) : (
          <p className="text-[13px] text-app-text">{label}</p>
        )}
        {hint && <p className="mt-0.5 text-[12px] leading-snug text-app-muted">{hint}</p>}
      </div>
      {children && <div className="flex shrink-0 items-center gap-2">{children}</div>}
    </div>
  );
}

/** A row whose control is a switch, labelled for screen readers. */
function ToggleRow({ label, hint, enabled, onChange, disabled }: {
  label: string;
  hint?: string;
  enabled: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <Row label={label} hint={hint}>
      <Toggle size="sm" enabled={enabled} onChange={onChange} disabled={disabled} aria-label={label} />
    </Row>
  );
}

/* ----------------------------------------------------------------------------
   Small leaf components
   ------------------------------------------------------------------------- */

function formatTimestamp(timestamp: number, locale: string): string {
  return new Date(timestamp).toLocaleString(locale, {
    month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit',
  });
}

/** Dictionary row — stable identity for memo via the `original` key. */
const DictionaryRow = memo(function DictionaryRow({
  entry, onDelete,
}: {
  entry: DictionaryEntry;
  onDelete: (original: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="group flex items-center gap-3 px-3.5 py-1.5 hover:bg-app-raised transition-colors duration-hover">
      <div className="flex-1 min-w-0 flex items-center gap-2 text-[12.5px]">
        <span className="text-app-muted truncate">{entry.original}</span>
        <ArrowRight className="size-3 shrink-0 text-app-faint" aria-hidden />
        <span className="text-app-text truncate">{entry.correction}</span>
      </div>
      <button
        type="button"
        onClick={() => onDelete(entry.original)}
        aria-label={t('common.delete')}
        className="opacity-0 group-hover:opacity-100 rounded-app-sm p-1 text-app-faint hover:text-app-danger transition"
      >
        <Trash2 className="size-3.5" aria-hidden />
      </button>
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

  return (
    <div className="group flex items-start gap-3 px-3.5 py-2 hover:bg-app-raised transition-colors duration-hover">
      <span className="shrink-0 whitespace-nowrap pt-px text-[11px] tabular-nums text-app-faint">
        {formatTimestamp(entry.timestamp, i18n.language)}
      </span>
      <p className="flex-1 min-w-0 text-[12.5px] leading-snug text-app-text line-clamp-2 break-words">{entry.text}</p>
      <div className="flex items-center opacity-0 group-hover:opacity-100 shrink-0">
        <Button variant="ghost" size="sm" onClick={handleReplay} loading={replaying} title={t('settings.history.replayTooltip')}>
          <Repeat className="size-3.5" />
        </Button>
        <Button variant="ghost" size="sm" onClick={handleCopy} title={t('settings.dictionary.copyTooltip')}>
          {copied ? <Check className="size-3.5 text-app-success" /> : <Copy className="size-3.5" />}
        </Button>
      </div>
    </div>
  );
});

/* ----------------------------------------------------------------------------
   Updates — one row; the details only appear when there is something to do
   ------------------------------------------------------------------------- */

function UpdatesRow() {
  const { t } = useTranslation();
  const { status, updateInfo, progress, error, checkForUpdates, downloadAndInstall, restartApp, dismiss } = useUpdater();
  const [appVersion, setAppVersion] = useState('…');
  const [buildSha, setBuildSha] = useState('');

  useEffect(() => { getVersion().then(setAppVersion).catch(() => {}); }, []);
  useEffect(() => {
    // The SHA is the ground truth on a beta — the marketing version stays put
    // across betas because the Windows MSI bundler rejects pre-release suffixes.
    invoke<{ commit_sha: string }>('get_build_info')
      .then((info) => setBuildSha(info.commit_sha))
      .catch(() => {});
  }, []);
  useTauriEvent('update-available', () => { checkForUpdates(); });

  const version = `v${appVersion}${buildSha && buildSha !== 'unknown' ? ` · ${buildSha}` : ''}`;

  const hint =
    status === 'up-to-date' ? t('settings.updates.upToDate', { version: appVersion })
    : status === 'checking' ? t('settings.updates.checking')
    : status === 'available' && updateInfo ? t('settings.updates.available', { version: updateInfo.version })
    : status === 'downloading' ? t('settings.updates.downloading', { progress: Math.round(progress) })
    : status === 'ready' ? t('settings.updates.ready')
    : status === 'error' ? (error || t('settings.updates.errorDefault'))
    : version;

  return (
    <Row label={t('settings.updates.title')} hint={<span className={cn(status === 'error' && 'text-app-danger')}>{hint}</span>}>
      {(status === 'idle' || status === 'up-to-date' || status === 'error') && (
        <Button variant="secondary" size="sm" onClick={checkForUpdates} leftIcon={<RefreshCw className="size-3" />}>
          {t('settings.updates.check')}
        </Button>
      )}
      {status === 'available' && (
        <>
          <Button variant="ghost" size="sm" onClick={dismiss}>{t('common.later')}</Button>
          <Button size="sm" onClick={downloadAndInstall} leftIcon={<Download className="size-3" />}>
            {t('settings.updates.install')}
          </Button>
        </>
      )}
      {status === 'ready' && <Button size="sm" onClick={restartApp}>{t('common.restartNow')}</Button>}
    </Row>
  );
}

/* ----------------------------------------------------------------------------
   Main Settings component
   ------------------------------------------------------------------------- */

export function Settings() {
  const { t } = useTranslation();
  const {
    aiPolishEnabled, screenContextEnabled, telemetryEnabled, shortcut, useBetaChannel,
    autostartEnabled, historyEnabled, vadAutoStopEnabled, vadSilenceSecs, audioDeviceName,
    transcriptionLanguage, diagnosticsEnabled, dictionary, history, loading, isPro, licenseKey,
    licenseStatus, licenseExpiresAt,
    licenseLoading, licenseError, soundPack,
    loadSettings, saveSettings, resetSettings, loadDictionary, deleteEntry,
    clearDictionary, loadHistory, clearHistory, loadLicense, activateLicense,
    deactivateLicense, validateLicense, loadUsage,
  } = useSettingsStore();

  const isMac = navigator.platform.startsWith('Mac');
  const [appVersion, setAppVersion] = useState('...');

  useEffect(() => { getVersion().then(setAppVersion).catch(() => {}); }, []);

  // Keep window title in sync with the active language. Deps include `t` so
  // the effect re-fires only on language change, not every render.
  useEffect(() => {
    try { getCurrentWindow().setTitle(t('windowTitle.settings')); }
    catch { /* not in Tauri (dev preview) */ }
  }, [t]);

  // An update found in the background opens the Advanced-free part of the
  // page anyway: the updates row is in General, near the top.
  const [showClearConfirm, setShowClearConfirm] = useState(false);
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [showClearHistoryConfirm, setShowClearHistoryConfirm] = useState(false);
  const [shortcutError, setShortcutError] = useState('');
  const [shortcutSuccess, setShortcutSuccess] = useState(false);
  // `null` until the keychain has answered, so a missing key is never
  // announced (or scrolled to) on a guess.
  const [hasGroqKey, setHasGroqKey] = useState<boolean | null>(null);
  const [groqKeySuccess, setGroqKeySuccess] = useState(false);
  const transcriptionSectionRef = useRef<HTMLDivElement>(null);
  const [newOriginal, setNewOriginal] = useState('');
  const [newCorrection, setNewCorrection] = useState('');
  const [addEntryError, setAddEntryError] = useState('');
  const [showRestartBanner, setShowRestartBanner] = useState(false);
  const [licenseInput, setLicenseInput] = useState('');
  const [showDeactivateConfirm, setShowDeactivateConfirm] = useState(false);
  const [showUninstallConfirm, setShowUninstallConfirm] = useState(false);
  const [uninstalling, setUninstalling] = useState(false);
  // The page shown, remembered between openings of the window.
  const [page, setPage] = useState<SettingsPage>(() => {
    try {
      const stored = localStorage.getItem(PAGE_STORAGE_KEY);
      return (PAGES.some((p) => p.id === stored) ? stored : 'dictation') as SettingsPage;
    } catch {
      return 'dictation';
    }
  });
  const selectPage = useCallback((next: SettingsPage) => {
    setPage(next);
    try { localStorage.setItem(PAGE_STORAGE_KEY, next); } catch { /* storage unavailable */ }
  }, []);
  // An update found in the background is shown on the page that holds it.
  useTauriEvent('update-available', () => selectPage('general'));

  // No local autostart state and no `isAutostartEnabled()` call — the plugin's
  // is_enabled() is unreliable on macOS for product names with spaces. The UI
  // reads `autostartEnabled` from the store (settings.json); the LaunchAgent
  // plist is the OS side-effect that the toggle keeps in sync.

  const handleAutostartToggle = async (enabled: boolean) => {
    try {
      // Real OS effect: register/unregister LaunchAgent plist (mac) /
      // registry key (Windows).
      if (enabled) await enableAutostart(); else await disableAutostart();
      // Persist the user's intent to settings.json so the UI doesn't depend
      // on the plugin's flaky is_enabled() readback.
      await saveSettings({ autostart_enabled: enabled });
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
    <K extends 'ai_polish_enabled' | 'screen_context_enabled' | 'telemetry_enabled' | 'history_enabled' | 'vad_auto_stop_enabled' | 'diagnostics_enabled'>(
      key: K,
      sideEffect?: () => void,
    ) => async (enabled: boolean) => {
      try {
        await saveSettings({ [key]: enabled } as Partial<Record<K, boolean>>);
        sideEffect?.();
      } catch (error) { console.error(`Failed to save ${key}:`, error); }
    },
    [saveSettings],
  );

  const handlePolishToggle = makeToggle('ai_polish_enabled');
  const handleScreenContextToggle = makeToggle('screen_context_enabled');

  /* Report a problem: a text file in Downloads plus a pre-addressed e-mail,
     built by `problem_report.rs`. The line under the button says where the
     file went, since attaching it is the one step we cannot do for them. */
  const [report, setReport] = useState<{ status: 'idle' | 'working' | 'done' | 'failed'; email?: string }>({ status: 'idle' });
  const handleReportProblem = useCallback(async () => {
    setReport({ status: 'working' });
    try {
      const result = await invoke<{ path: string; email: string }>('report_problem');
      setReport({ status: 'done', email: result.email });
    } catch (error) {
      console.error('Failed to create problem report:', error);
      setReport({ status: 'failed' });
    }
  }, []);
  const handleTelemetryToggle = makeToggle('telemetry_enabled', () => setShowRestartBanner(true));
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
  // Never before the microphone is granted: listing input devices is what
  // made macOS raise its prompt at launch, over the onboarding.
  const refreshAudioDevices = useCallback(() => {
    invoke<string>('check_microphone_permission')
      .then((status) => {
        if (status !== 'Granted') return;
        return invoke<Array<{ name: string; is_default: boolean }>>('list_audio_input_devices')
          .then(setAudioDevices);
      })
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

  // Settings is where TTP sends a user with no key (at launch, or from a
  // dictation that could not run), so take them straight to the form.
  const scrolledToKey = useRef(false);
  useEffect(() => {
    if (hasGroqKey !== false || scrolledToKey.current) return;
    scrolledToKey.current = true;
    selectPage('general');
    setTimeout(() => transcriptionSectionRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' }), 50);
  }, [hasGroqKey, selectPage]);

  const handleGroqKeySaved = () => {
    setHasGroqKey(true); setGroqKeySuccess(true);
    setTimeout(() => setGroqKeySuccess(false), 3000);
  };

  // Windows/Linux only: macOS uses TriggerPicker and the event tap.
  const handleShortcutChange = async (newShortcut: string) => {
    setShortcutError(''); setShortcutSuccess(false);
    try {
      await invoke('update_shortcut_cmd', { shortcut: newShortcut });
      await saveSettings({ shortcut: newShortcut });
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
    } catch (error) { console.error('Activation failed:', error); }
  };

  const handleDeactivateLicense = async () => {
    try {
      await deactivateLicense(); setShowDeactivateConfirm(false);
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

  // The Companion catalogue. Locked packs are listed on purpose — you should
  // be able to hear what you might buy, and a list that hides its contents
  // cannot tempt anyone.
  //
  // THREE STATES, NOT TWO. `null` means "the answer has not arrived"; a value
  // means the backend answered; `companionError` means it could not be
  // reached. These used to collapse into two: a failed `list_sound_packs`
  // called `setSoundPacks([])` and a failed `cosmetics_unlocked` called
  // `setCosmeticsUnlocked(false)`, so an IPC failure was indistinguishable
  // from an honest empty catalogue and from not having bought anything. The
  // panel then hid itself on `packs.length === 0`. A paying customer whose
  // IPC call failed was shown, with no error anywhere, the exact UI that says
  // "you do not own this" — the app accusing its own buyer of not paying.
  const [soundPacks, setSoundPacks] = useState<SoundPack[] | null>(null);
  const [cosmeticsUnlocked, setCosmeticsUnlocked] = useState<boolean | null>(null);
  const [companionError, setCompanionError] = useState(false);

  const loadCompanion = useCallback(() => {
    setCompanionError(false);
    // Both calls must land before the panel can say anything true: the
    // catalogue without the entitlement would paint every paid pack as
    // locked. Either failing is one failure, reported once.
    Promise.all([
      invoke<SoundPack[]>('list_sound_packs'),
      invoke<boolean>('cosmetics_unlocked'),
    ])
      .then(([packs, unlocked]) => {
        setSoundPacks(packs);
        setCosmeticsUnlocked(unlocked);
      })
      .catch((e) => {
        console.error('companion:', e);
        setSoundPacks(null);
        setCosmeticsUnlocked(null);
        setCompanionError(true);
      });
  }, []);

  useEffect(() => { loadCompanion(); }, [isPro, loadCompanion]);

  const handleSelectPack = useCallback(async (id: string) => {
    try { await saveSettings({ sound_pack: id }); } catch (e) { console.error('sound_pack:', e); }
  }, [saveSettings]);

  // No caps, so no "at cap" states, no trial countdown, and no x/y rows
  // counting down to a paywall. Every feature is free and unlimited; a
  // licence is a thank-you, not a key.

  // Windows/Linux only — macOS picks any key through TriggerPicker.
  const triggerOptions = [
    { value: 'Ctrl+Space', label: t('settings.recordingTrigger.optionCtrlSpace'), desc: t('settings.recordingTrigger.descRecommended'), recommended: true },
    { value: 'Ctrl+Shift+Space', label: t('settings.recordingTrigger.optionCtrlShiftSpace'), desc: '', recommended: false },
    { value: 'Super+J', label: t('settings.recordingTrigger.optionWinJ'), desc: t('settings.recordingTrigger.descNoConflicts'), recommended: false },
  ];

  const translateIfKey = (s: string | null): string =>
    s && (s.startsWith('error.') || s.startsWith('permission.')) ? t(s) : (s ?? '');
  const showShortcutError = translateIfKey(shortcutError);
  const showLicenseError = translateIfKey(licenseError);

  const [pendingBetaDowngrade, setPendingBetaDowngrade] = useState(false);
  const persistBeta = async (enabled: boolean) => {
    try { await saveSettings({ use_beta_channel: enabled }); }
    catch (error) { console.error('Failed to save update channel setting:', error); }
  };
  const handleBetaToggle = (enabled: boolean) => {
    // Leaving the beta channel while running a beta build downgrades on the
    // next update: say so first.
    if (!enabled && appVersion.includes('-beta')) { setPendingBetaDowngrade(true); return; }
    void persistBeta(enabled);
  };

  const selectedPack = soundPacks?.find((p) => p.id === soundPack) ?? soundPacks?.[0];

  return (
    <div className="h-screen flex bg-app-bg text-app-text">
      <SettingsSidebar page={page} onSelect={selectPage} version={appVersion} />

      <main className="flex-1 min-w-0 overflow-y-auto bg-noise">
        <div className="mx-auto max-w-[540px] px-7 pt-6 pb-10">
          <h1 className="mb-5 text-[20px] font-semibold tracking-[-0.01em] text-app-text">
            {t(`settings.pages.${page}`)}
          </h1>

          <PermissionBanner />

          {page === 'dictation' && (
            <>
            <Group title={t('settings.groups.shortcuts')}>
              <Row
                label={t('settings.recordingTrigger.title')}
                hint={t('settings.recordingTrigger.hintShort')}
              >
                {isMac ? (
                  <TriggerPicker compact />
                ) : (
                  <select
                    className={SELECT}
                    value={shortcut}
                    onChange={(e) => handleShortcutChange(e.target.value)}
                    disabled={loading}
                    aria-label={t('settings.recordingTrigger.title')}
                  >
                    {triggerOptions.map((opt) => <option key={opt.value} value={opt.value}>{opt.label}</option>)}
                  </select>
                )}
              </Row>
              {isMac && (
                <Row
                  label={t('settings.recordingTrigger.secondaryTitle')}
                  hint={t('settings.recordingTrigger.secondaryHint')}
                >
                  <TriggerPicker compact slot="second" />
                </Row>
              )}
            </Group>

            <Group title={t('settings.groups.recording')}>
              <Row label={t('settings.recordingMode.audioDeviceLabel')} htmlFor="audio-device-select">
                <select
                  id="audio-device-select"
                  className={SELECT}
                  value={audioDeviceName ?? ''}
                  onChange={(e) => handleAudioDeviceChange(e.target.value)}
                  onFocus={refreshAudioDevices}
                  disabled={loading}
                >
                  <option value="">{t('settings.recordingMode.audioDeviceDefault')}</option>
                  {audioDevices.map((d) => (
                    <option key={d.name} value={d.name}>
                      {d.is_default ? `${d.name} ${t('settings.recordingMode.audioDeviceDefaultSuffix')}` : d.name}
                    </option>
                  ))}
                </select>
              </Row>
              <Row label={t('settings.transcription.languageLabel')} hint={t('settings.transcription.languageHint')} htmlFor="transcription-language-select">
                <select
                  id="transcription-language-select"
                  className={SELECT}
                  value={transcriptionLanguage}
                  onChange={(e) =>
                    saveSettings({
                      transcription_language: e.target.value === 'auto' ? null : e.target.value,
                    }).catch((err) => console.error('Failed to save transcription_language:', err))
                  }
                  disabled={loading}
                >
                  <option value="auto">{t('settings.transcription.languageAuto')}</option>
                  <option value="en">{t('settings.transcription.languageEnglish')}</option>
                  <option value="fr">{t('settings.transcription.languageFrench')}</option>
                </select>
              </Row>
              <ToggleRow
                label={t('settings.transcription.polishLabel')}
                hint={t('settings.transcription.polishDesc')}
                enabled={aiPolishEnabled}
                onChange={handlePolishToggle}
                disabled={loading}
              />
              {aiPolishEnabled && (
                <ToggleRow
                  label={t('settings.transcription.screenContextLabel')}
                  hint={t('settings.transcription.screenContextDesc')}
                  enabled={screenContextEnabled}
                  onChange={handleScreenContextToggle}
                  disabled={loading}
                />
              )}
              {shortcutError && <p className="px-3.5 py-2 text-[12px] text-app-danger">{showShortcutError}</p>}
              {shortcutSuccess && <p className="px-3.5 py-2 text-[12px] text-app-success">{t('settings.recordingTrigger.successUpdated')}</p>}
            </Group>
            {isMac && <FnEmojiNudge />}
            </>
          )}

          {page === 'general' && (
            <>
            <div ref={transcriptionSectionRef} className="scroll-mt-6">
              <Group title={t('settings.transcription.groqLabel')}>
                {hasGroqKey === false ? (
                  <div className="px-3.5 py-3">
                    <ApiKeyForm compact onSuccess={handleGroqKeySaved} submitLabel={t('common.save')} />
                  </div>
                ) : (
                  <Row
                    label={t('settings.transcription.keyLabel')}
                    hint={groqKeySuccess ? t('settings.transcription.keySaved') : undefined}
                  >
                    <span className="text-[12px] text-app-success">{hasGroqKey ? t('settings.transcription.keyConfigured') : '…'}</span>
                    <Button variant="ghost" size="sm" onClick={() => setHasGroqKey(false)}>{t('common.change')}</Button>
                  </Row>
                )}
              </Group>
            </div>

            <Group title={t('settings.groups.app')}>
              <ToggleRow
                label={t('settings.recordingMode.launchStartupLabel')}
                enabled={autostartEnabled}
                onChange={handleAutostartToggle}
                disabled={loading}
              />
              <UpdatesRow />
            </Group>
            </>
          )}

          {page === 'dictionary' && (
            <Group
              title={t('settings.dictionary.title')}
              action={dictionary.length > 0 ? (
                <button type="button" onClick={() => setShowClearConfirm(true)} className="text-[11.5px] text-app-faint hover:text-app-danger transition-colors">
                  {t('common.clearAll')}
                </button>
              ) : undefined}
            >
              <form
                className="flex items-center gap-2 px-3.5 py-2"
                onSubmit={(e) => { e.preventDefault(); void handleAddEntry(); }}
              >
                <Input
                  value={newOriginal}
                  onChange={(e) => setNewOriginal(e.target.value)}
                  placeholder={t('settings.dictionary.placeholderMisheard')}
                  aria-label={t('settings.dictionary.labelMisheard')}
                  className="h-7 flex-1 text-[12.5px]"
                />
                <ArrowRight className="size-3.5 shrink-0 text-app-faint" aria-hidden />
                <Input
                  value={newCorrection}
                  onChange={(e) => setNewCorrection(e.target.value)}
                  placeholder={t('settings.dictionary.placeholderCorrection')}
                  aria-label={t('settings.dictionary.labelCorrection')}
                  className="h-7 flex-1 text-[12.5px]"
                />
                <Button type="submit" size="sm" variant="secondary" disabled={!newOriginal.trim() || !newCorrection.trim()}>
                  {t('common.add')}
                </Button>
              </form>
              {addEntryError && <p className="px-3.5 py-1.5 text-[12px] text-app-danger">{addEntryError}</p>}
              {dictionary.length === 0 ? (
                <p className="px-3.5 py-2.5 text-[12px] text-app-muted">{t('settings.dictionary.emptyState')}</p>
              ) : (
                <div className="max-h-44 overflow-y-auto">
                  {dictionary.map((entry) => (
                    <DictionaryRow key={entry.original} entry={entry} onDelete={handleDeleteEntry} />
                  ))}
                </div>
              )}
            </Group>
          )}

          {page === 'history' && (
            <Group
              title={t('settings.history.title')}
              action={history.length > 0 ? (
                <button type="button" onClick={() => setShowClearHistoryConfirm(true)} className="text-[11.5px] text-app-faint hover:text-app-danger transition-colors">
                  {t('settings.history.clearButton')}
                </button>
              ) : undefined}
            >
              <ToggleRow
                label={t('settings.history.saveLabel')}
                enabled={historyEnabled}
                onChange={handleHistoryEnabledToggle}
                disabled={loading}
              />
              {history.length === 0 ? (
                <p className="px-3.5 py-2.5 text-[12px] text-app-muted">
                  {historyEnabled ? t('settings.history.emptyEnabled') : t('settings.history.emptyDisabled')}
                </p>
              ) : (
                <div className="max-h-64 overflow-y-auto">
                  {history.map((entry) => <HistoryRow key={entry.timestamp} entry={entry} />)}
                </div>
              )}
            </Group>
          )}

          {page === 'support' && (
            <Group title={t('settings.pro.title')}>
              {companionError ? (
                <div className="px-3.5 py-2.5">
                  <Banner
                    tone="warning"
                    title={t('error.companion_unreachable_title')}
                    action={<button type="button" onClick={loadCompanion} className="text-[12px] font-medium text-app-accent hover:underline">{t('error.retry')}</button>}
                  >
                    {t('error.companion_unreachable')}
                  </Banner>
                </div>
              ) : soundPacks && soundPacks.length > 0 && (
                <Row
                  label={t('settings.companion.soundsLabel')}
                  hint={selectedPack ? t(`settings.companion.packs.${selectedPack.id}.desc`) : undefined}
                  htmlFor="sound-pack-select"
                >
                  <select
                    id="sound-pack-select"
                    className={SELECT}
                    value={soundPack}
                    onChange={(e) => handleSelectPack(e.target.value)}
                    disabled={loading}
                  >
                    {soundPacks.map((pack) => {
                      // `!== true`, not `!unlocked`: an unknown entitlement must
                      // not be rendered as a lock.
                      const locked = !pack.free && cosmeticsUnlocked !== true;
                      return (
                        <option key={pack.id} value={pack.id} disabled={locked}>
                          {t(`settings.companion.packs.${pack.id}.name`)}{locked ? ' 🔒' : ''}
                        </option>
                      );
                    })}
                  </select>
                  <button
                    type="button"
                    onClick={() => { invoke('preview_sound_pack', { packId: soundPack }).catch(() => {}); }}
                    aria-label={t('settings.companion.preview')}
                    title={t('settings.companion.preview')}
                    className="rounded-app-sm p-1.5 text-app-muted hover:text-app-text hover:bg-app-raised transition-colors"
                  >
                    <Volume2 className="size-3.5" aria-hidden />
                  </button>
                </Row>
              )}
              {!isPro ? (
                <>
                  <div className="flex items-center gap-2 px-3.5 py-2">
                    <Input
                      value={licenseInput}
                      onChange={(e) => setLicenseInput(e.target.value)}
                      placeholder={t('settings.pro.inputPlaceholder')}
                      spellCheck={false}
                      disabled={licenseLoading}
                      className="h-7 flex-1 font-mono text-[12px]"
                      onKeyDown={(e) => { if (e.key === 'Enter' && licenseInput.trim()) handleActivateLicense(); }}
                      aria-label={t('settings.pro.labelKey')}
                    />
                    <Button size="sm" variant="secondary" onClick={handleActivateLicense} disabled={licenseLoading || !licenseInput.trim()} loading={licenseLoading}>
                      {t('settings.pro.activateShort')}
                    </Button>
                  </div>
                  {licenseError && <p className="px-3.5 pb-2 text-[12px] text-app-danger">{showLicenseError}</p>}
                  <div className="flex items-center justify-between gap-3 px-3.5 py-2">
                    <p className="text-[11.5px] leading-snug text-app-muted">{t('settings.support.hint')}</p>
                    <a
                      href="https://amirks.lemonsqueezy.com/checkout/buy/23ded1c4-c862-4f8c-ada5-0bb3dc2e0060"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="shrink-0 text-[12px] font-medium text-app-accent hover:text-app-accent-hover"
                    >
                      {t('settings.pro.buyLink')}
                    </a>
                  </div>
                </>
              ) : (
                <Row
                  label={<span className="inline-flex items-center gap-1.5">{t('settings.pro.labelKey')} <span className="font-mono text-app-muted">{maskedLicenseKey}</span></span>}
                  hint={`${licenseStatus ?? t('settings.pro.statusUnknown')} · ${formatExpiry(licenseExpiresAt)}`}
                >
                  <Button variant="ghost" size="sm" onClick={validateLicense} disabled={licenseLoading} title={t('common.refresh')}>
                    <RefreshCw className="size-3" />
                  </Button>
                  <Button variant="ghost" size="sm" onClick={() => setShowDeactivateConfirm(true)} disabled={licenseLoading} className="text-app-danger hover:text-app-danger">
                    {t('settings.pro.deactivateShort')}
                  </Button>
                </Row>
              )}
            </Group>
          )}

          {page === 'advanced' && (
            <Group title={t('settings.groups.troubleshooting')}>
              <Row
                label={t('settings.report.label')}
                hint={
                  report.status === 'done'
                    ? t('settings.report.done', { email: report.email })
                    : report.status === 'failed'
                      ? t('settings.report.failed')
                      : t('settings.report.hint')
                }
              >
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={handleReportProblem}
                  disabled={report.status === 'working'}
                >
                  {t('settings.report.button')}
                </Button>
              </Row>
              <ToggleRow
                label={t('settings.recordingMode.vadAutoStopLabel')}
                hint={t('settings.recordingMode.vadAutoStopDesc')}
                enabled={vadAutoStopEnabled}
                onChange={handleVadAutoStopToggle}
                disabled={loading}
              />
              {vadAutoStopEnabled && (
                <Row label={t('settings.recordingMode.vadSilenceSecsLabel')} htmlFor="vad-silence-secs">
                  <input
                    id="vad-silence-secs"
                    type="range"
                    min={1}
                    max={10}
                    step={1}
                    value={vadSilenceSecs}
                    onChange={(e) => handleVadSilenceSecsChange(Number(e.target.value))}
                    disabled={loading}
                    className="w-32 accent-app-accent"
                  />
                  <span className="w-7 text-right text-[12px] tabular-nums text-app-text">{vadSilenceSecs}s</span>
                </Row>
              )}
              <ToggleRow
                label={t('settings.privacy.helpLabel')}
                hint={t('settings.privacy.whatSentBody')}
                enabled={telemetryEnabled}
                onChange={handleTelemetryToggle}
                disabled={loading}
              />
              {showRestartBanner && (
                <Row label={t('settings.privacy.restartHint')}>
                  <Button variant="secondary" size="sm" onClick={() => relaunch().catch(console.error)}>
                    {t('common.restartNow')}
                  </Button>
                </Row>
              )}
              <ToggleRow
                label={t('settings.updateChannel.labelBeta')}
                hint={useBetaChannel ? t('settings.updateChannel.warning') : t('settings.updateChannel.descBeta')}
                enabled={useBetaChannel}
                onChange={handleBetaToggle}
                disabled={loading}
              />
              <ToggleRow
                label={t('settings.diagnostics.label')}
                enabled={diagnosticsEnabled}
                onChange={handleDiagnosticsToggle}
                disabled={loading}
              />
              <Row label={t('settings.logs.title')}>
                <Button variant="secondary" size="sm" onClick={() => { invoke('reveal_log_folder').catch(console.error); }}>
                  {t('settings.logs.buttonShort')}
                </Button>
                <Button variant="secondary" size="sm" onClick={() => { invoke('reveal_recordings_folder').catch(console.error); }}>
                  {t('settings.logs.recordingsButtonShort')}
                </Button>
              </Row>
              <Row label={t('settings.reset.title')} hint={t('settings.reset.desc')}>
                <Button variant="secondary" size="sm" onClick={() => setShowResetConfirm(true)} className="text-app-danger">
                  {t('settings.reset.button')}
                </Button>
              </Row>
              <Row label={t('settings.uninstall.title')} hint={t('settings.uninstall.desc')}>
                <Button variant="danger" size="sm" onClick={() => setShowUninstallConfirm(true)}>
                  {t('settings.uninstall.buttonShort')}
                </Button>
              </Row>
            </Group>
          )}

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
          <ConfirmDialog
            open={pendingBetaDowngrade}
            title={t('dialog.downgradeBeta.title')}
            message={t('dialog.downgradeBeta.message')}
            confirmText={t('common.continue')}
            cancelText={t('common.cancel')}
            tone="warning"
            onConfirm={() => { setPendingBetaDowngrade(false); void persistBeta(false); }}
            onCancel={() => setPendingBetaDowngrade(false)}
          />


          <WhatsNew />
        </div>
      </main>
    </div>
  );
}

interface SoundPack {
  id: string;
  free: boolean;
}

/* ----------------------------------------------------------------------------
   Pages and the sidebar
   ------------------------------------------------------------------------- */

type SettingsPage = 'dictation' | 'general' | 'dictionary' | 'history' | 'support' | 'advanced';

const PAGE_STORAGE_KEY = 'ttp-settings-page';

const PAGES: { id: SettingsPage; icon: typeof Mic }[] = [
  { id: 'dictation', icon: Mic },
  { id: 'general', icon: SlidersHorizontal },
  { id: 'dictionary', icon: BookOpen },
  { id: 'history', icon: History },
  { id: 'support', icon: Heart },
  { id: 'advanced', icon: Wrench },
];

function SettingsSidebar({ page, onSelect, version }: {
  page: SettingsPage;
  onSelect: (page: SettingsPage) => void;
  version: string;
}) {
  const { t } = useTranslation();
  return (
    <aside className="flex w-[190px] shrink-0 flex-col border-r border-app-border bg-app-dim">
      <div className="flex items-center gap-2.5 px-4 pt-5 pb-4">
        <BrandTile size="sm" />
        <div className="min-w-0">
          <p className="text-[13px] font-semibold leading-tight text-app-text">TTP</p>
          <p className="text-[11px] tabular-nums text-app-faint">v{version}</p>
        </div>
      </div>

      <nav className="flex-1 px-2">
        <ul className="space-y-0.5">
          {PAGES.map(({ id, icon: Icon }) => {
            const active = page === id;
            return (
              <li key={id}>
                <button
                  type="button"
                  onClick={() => onSelect(id)}
                  aria-current={active ? 'page' : undefined}
                  className={cn(
                    'flex w-full items-center gap-2.5 rounded-app-sm px-2.5 py-1.5 text-left text-[13px]',
                    'transition-colors duration-hover',
                    active ? 'bg-app-accent text-app-accent-fg' : 'text-app-text hover:bg-app-raised',
                  )}
                >
                  <Icon className={cn('size-4 shrink-0', active ? 'text-app-accent-fg' : 'text-app-muted')} strokeWidth={1.75} aria-hidden />
                  {t(`settings.pages.${id}`)}
                </button>
              </li>
            );
          })}
        </ul>
      </nav>

      <MonthStats />

      <div className="flex items-center gap-2 border-t border-app-border px-4 py-3 text-[11px] text-app-faint">
        <a href="https://amirks.eu" target="_blank" rel="noopener noreferrer" className="hover:text-app-text transition-colors">amirks.eu</a>
        <span aria-hidden>·</span>
        <a href="https://www.linkedin.com/in/amirks/" target="_blank" rel="noopener noreferrer" className="hover:text-app-text transition-colors">LinkedIn</a>
      </div>
    </aside>
  );
}

/* ----------------------------------------------------------------------------
   MonthStats — this month's words, in the header. Refreshes when a
   transcription completes.
   ------------------------------------------------------------------------- */

interface AnalyticsWindowData { transcriptions: number; words: number; chars: number; }
interface AnalyticsSummary {
  month: AnalyticsWindowData;
  all_time: AnalyticsWindowData;
}

function MonthStats() {
  const { t, i18n } = useTranslation();
  const [summary, setSummary] = useState<AnalyticsSummary | null>(null);

  const load = useCallback(() => {
    invoke<AnalyticsSummary>('get_analytics_summary').then(setSummary).catch(() => {});
  }, []);

  useEffect(() => { load(); }, [load]);
  useTauriEvent<string>('recording-state-changed', (event) => {
    if (event.payload === 'Idle') setTimeout(load, 600);
  });

  if (!summary || summary.all_time.transcriptions === 0) return null;
  const fmt = new Intl.NumberFormat(i18n.language, { notation: 'compact', maximumFractionDigits: 1 }).format;

  return (
    <div className="border-t border-app-border px-4 py-3">
      <p className="text-[10.5px] font-semibold uppercase tracking-[0.06em] text-app-faint">{t('settings.sidebar.thisMonth')}</p>
      <p className="mt-1 text-[15px] font-semibold leading-tight tabular-nums text-app-text">
        {fmt(summary.month.words)} <span className="text-[11px] font-normal text-app-muted">{t('settings.sidebar.words')}</span>
      </p>
      <p className="text-[11px] text-app-faint">{t('settings.sidebar.transcriptions', { count: summary.month.transcriptions })}</p>
    </div>
  );
}

export default Settings;
