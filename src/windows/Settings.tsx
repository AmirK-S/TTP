// TTP - Talk To Paste
// Settings window - configure app behavior and manage dictionary

import { useEffect, useState, useCallback, useRef, memo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Copy, Check, Download, RefreshCw, Crown, Loader2,
  Activity, User, Mic, Languages, BookOpen, Clock, Globe, SlidersHorizontal,
} from 'lucide-react';
import { cn } from '../lib/cn';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useTauriEvent } from '../hooks/useTauriEvent';
import { getVersion } from '@tauri-apps/api/app';
import { relaunch } from '@tauri-apps/plugin-process';
import { enable as enableAutostart, disable as disableAutostart, isEnabled as isAutostartEnabled } from '@tauri-apps/plugin-autostart';
import { trackEvent } from '../lib/analytics';
import { useUpdater } from '../hooks/useUpdater';
import { useSettingsStore, DictionaryEntry, HistoryEntry } from '../stores/settings-store';
import { PermissionBanner } from '../components/PermissionBanner';
import type { LanguageChoice } from '../i18n/config';
import WhatsNew from '../components/WhatsNew';

/**
 * Toggle switch component for settings
 */
function Toggle({
  enabled,
  onChange,
  disabled = false,
}: {
  enabled: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={() => !disabled && onChange(!enabled)}
      className={`
        relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent
        transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-app-accent focus:ring-offset-2
        ${enabled ? 'bg-app-accent' : 'bg-app-raised'}
        ${disabled ? 'opacity-50 cursor-not-allowed' : ''}
      `}
      disabled={disabled}
      role="switch"
      aria-checked={enabled}
    >
      <span
        className={`
          pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0
          transition duration-200 ease-in-out
          ${enabled ? 'translate-x-5' : 'translate-x-0'}
        `}
      />
    </button>
  );
}

/**
 * Confirmation dialog component
 */
function ConfirmDialog({
  open,
  title,
  message,
  confirmText,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title: string;
  message: string;
  confirmText: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-app-surface rounded-lg shadow-xl p-6 max-w-sm mx-4">
        <h3 className="text-lg font-semibold text-app-text mb-2">
          {title}
        </h3>
        <p className="text-app-muted mb-4">{message}</p>
        <div className="flex justify-end gap-3">
          <button
            onClick={onCancel}
            className="px-4 py-2 text-sm font-medium text-app-text hover:bg-app-raised rounded-md transition-colors"
          >
            {t('common.cancel')}
          </button>
          <button
            onClick={onConfirm}
            className="px-4 py-2 text-sm font-medium text-white bg-red-600 hover:bg-red-700 rounded-md transition-colors"
          >
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * Dictionary table row component (memoised).
 *
 * onDelete takes the entry's `original` so callers can pass a stable callback
 * — without that, every parent re-render would create a fresh closure and
 * defeat React.memo.
 */
const DictionaryRow = memo(function DictionaryRow({
  entry,
  onDelete,
}: {
  entry: DictionaryEntry;
  onDelete: (original: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <tr className="border-b border-app-border">
      <td className="py-3 px-4 text-app-text font-mono text-sm">
        {entry.original}
      </td>
      <td className="py-3 px-4 text-app-text font-mono text-sm">
        {entry.correction}
      </td>
      <td className="py-3 px-4 text-right">
        <button
          onClick={() => onDelete(entry.original)}
          className="text-red-600 hover:text-red-700 text-sm font-medium"
        >
          {t('common.delete')}
        </button>
      </td>
    </tr>
  );
});

/**
 * Format timestamp to readable date string
 */
function formatTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  return date.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  });
}

/**
 * History entry row component
 */
const HistoryRow = memo(function HistoryRow({ entry }: { entry: HistoryEntry }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(entry.text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (error) {
      console.error('Failed to copy:', error);
    }
  };

  // Preview: first ~100 characters with ellipsis
  const preview =
    entry.text.length > 100 ? entry.text.slice(0, 100) + '...' : entry.text;

  return (
    <div className="flex items-start gap-3 p-3 odd:bg-app-raised">
      <div className="flex-1 min-w-0">
        <p className="text-xs text-app-muted mb-1">
          {formatTimestamp(entry.timestamp)}
        </p>
        <p className="text-sm text-app-text break-words">
          {preview}
        </p>
      </div>
      <button
        onClick={handleCopy}
        className="flex-shrink-0 p-2 text-app-muted hover:text-app-text rounded-md hover:bg-app-raised transition-colors"
        title={t('settings.dictionary.copyTooltip')}
      >
        {copied ? (
          <Check className="w-4 h-4 text-green-500" />
        ) : (
          <Copy className="w-4 h-4" />
        )}
      </button>
    </div>
  );
});

/**
 * Update Channel section — lets the user opt into beta builds.
 *
 * Toggling OFF while running a beta build shows a warning: until the next
 * stable release catches up, the user will not receive updates (their build
 * is "ahead" of stable). We use window.confirm intentionally — the rest of
 * the app uses a custom ConfirmDialog, but wiring that in here would require
 * refactoring the modal out of the parent Settings component. The native
 * dialog is acceptable for an admin-style setting; promoting it to the
 * shared ConfirmDialog is a TODO.
 */
interface AnalyticsWindowData {
  transcriptions: number;
  words: number;
  chars: number;
}

interface AnalyticsSummary {
  week: AnalyticsWindowData;
  month: AnalyticsWindowData;
  all_time: AnalyticsWindowData;
  daily: Array<{ date: string; transcriptions: number; words: number; chars: number }>;
}

/**
 * Local-only analytics panel — pulls aggregated counts from usage.json
 * via `get_analytics_summary`. No network, no third-party plugin (we
 * dropped the one that crashed). User sees their own usage; nothing
 * leaves the machine.
 */
function AnalyticsSection() {
  const { t, i18n } = useTranslation();
  const [summary, setSummary] = useState<AnalyticsSummary | null>(null);

  const loadSummary = useCallback(() => {
    invoke<AnalyticsSummary>('get_analytics_summary')
      .then(setSummary)
      .catch((e) => console.error('[Analytics] load failed:', e));
  }, []);

  useEffect(() => {
    loadSummary();
  }, [loadSummary]);

  // Re-fetch when a transcription finishes so the panel updates live
  // without the user having to close + reopen Settings. The pipeline
  // transitions back to Idle right after `record_transcription` persists
  // today's bucket, so by the time we re-invoke `get_analytics_summary`
  // the new data is already on disk. Small delay to let the file write
  // settle (mirrors the loadHistory/loadUsage pattern below).
  useTauriEvent<string>('recording-state-changed', (event) => {
    if (event.payload === 'Idle') {
      setTimeout(loadSummary, 600);
    }
  });

  // Compact notation (1.2K, 234K, 1.2M) keeps the column readable at any
  // scale — a year-long power user can hit millions of words otherwise the
  // full "1 234 567" overflows the 3-col grid. Locale-aware: French uses
  // "1,2 k" / "1,2 M", English uses "1.2K" / "1.2M".
  const compactFormatter = new Intl.NumberFormat(i18n.language, {
    notation: 'compact',
    maximumFractionDigits: 1,
  });
  const fmt = (n: number) => compactFormatter.format(n);

  const StatCol = ({ title, stats }: { title: string; stats: AnalyticsWindowData }) => (
    <div className="text-center">
      <p className="text-[10px] font-medium uppercase tracking-wider text-app-muted">
        {title}
      </p>
      <p className="text-xl font-semibold text-app-text mt-1 leading-tight">
        {fmt(stats.words)}
      </p>
      <p className="text-[11px] text-app-muted">
        {fmt(stats.transcriptions)} {t('settings.analytics.transcriptions')}
      </p>
    </div>
  );

  return (
    <section className="bg-app-surface rounded-app-lg shine-sm border border-app-border px-4 py-3 mb-6">
      <div className="flex items-baseline justify-between mb-2">
        <h2 className="text-sm font-semibold text-app-text">
          {t('settings.analytics.title')}
        </h2>
        <span className="text-[10px] text-app-faint">
          {t('settings.analytics.wordsHint')}
        </span>
      </div>
      {summary === null ? (
        <p className="text-xs text-app-muted text-center py-3">
          {t('settings.analytics.loading')}
        </p>
      ) : summary.all_time.transcriptions === 0 ? (
        <p className="text-xs text-app-muted text-center py-3">
          {t('settings.analytics.empty')}
        </p>
      ) : (
        <div className="grid grid-cols-3 gap-2">
          <StatCol title={t('settings.analytics.thisWeek')} stats={summary.week} />
          <StatCol title={t('settings.analytics.thisMonth')} stats={summary.month} />
          <StatCol title={t('settings.analytics.allTime')} stats={summary.all_time} />
        </div>
      )}
    </section>
  );
}

function UpdateChannelSection() {
  const { t } = useTranslation();
  const { useBetaChannel, saveSettings, loading } = useSettingsStore();
  const [appVersion, setAppVersion] = useState('...');

  useEffect(() => {
    getVersion().then((v) => setAppVersion(v)).catch(() => {});
  }, []);

  const isOnBetaBuild = appVersion.includes('-beta');

  const handleToggle = async (enabled: boolean) => {
    // Downgrade safeguard: if the user is currently on a beta build and
    // disabling beta, warn them that they are "ahead of" the stable channel
    // and will see no updates until a new stable release ships.
    if (!enabled && isOnBetaBuild) {
      // TODO: replace window.confirm with the shared ConfirmDialog component
      // once we refactor it out of the parent Settings scope.
      const ok = window.confirm(t('dialog.downgradeBeta.message'));
      if (!ok) return;
    }

    try {
      await saveSettings({ use_beta_channel: enabled });
      trackEvent('setting_changed', {
        setting_name: 'use_beta_channel',
        new_value: String(enabled),
      });
    } catch (error) {
      console.error('Failed to save update channel setting:', error);
    }
  };

  return (
    <section className="bg-app-surface rounded-app-lg shine-sm border border-app-border p-6 mb-6">
      <div className="flex items-center justify-between mb-2">
        <h2 className="text-lg font-semibold text-app-text">
          {t('settings.updateChannel.title')}
        </h2>
        {isOnBetaBuild && (
          <span className="px-2 py-0.5 text-xs font-semibold rounded-full bg-purple-100 text-purple-800 dark:bg-purple-900/40 dark:text-purple-300">
            {t('settings.updateChannel.betaBadge')}
          </span>
        )}
      </div>

      <div className="flex items-center justify-between mt-4">
        <div className="flex-1 pr-4">
          <p className="text-app-text font-medium">
            {useBetaChannel ? t('settings.updateChannel.labelBeta') : t('settings.updateChannel.labelStable')}
          </p>
          <p className="text-sm text-app-muted mt-1">
            {useBetaChannel
              ? t('settings.updateChannel.descBeta')
              : t('settings.updateChannel.descStable')}
          </p>
        </div>
        <Toggle
          enabled={useBetaChannel}
          onChange={handleToggle}
          disabled={loading}
        />
      </div>

      {useBetaChannel && (
        <div className="mt-4 p-3 bg-app-warning-tint rounded-lg">
          <p className="text-sm text-amber-700 dark:text-amber-400">
            {t('settings.updateChannel.warning')}
          </p>
        </div>
      )}
    </section>
  );
}

/**
 * Update section component
 */
function UpdateSection() {
  const { t } = useTranslation();
  const {
    status,
    updateInfo,
    progress,
    error,
    checkForUpdates,
    downloadAndInstall,
    restartApp,
    dismiss,
  } = useUpdater();

  const [appVersion, setAppVersion] = useState('...');

  useEffect(() => {
    getVersion().then(v => setAppVersion(v)).catch(() => {});
  }, []);

  // Auto-trigger check when main window detects an update and emits the event
  useTauriEvent('update-available', () => {
    checkForUpdates();
  });

  return (
    <section className="bg-app-surface rounded-app-lg shine-sm border border-app-border p-6 mb-6">
      <h2 className="text-lg font-semibold text-app-text mb-4">
        {t('settings.updates.title')}
      </h2>

      <div className="space-y-3">
        {status === 'idle' && (
          <button
            onClick={checkForUpdates}
            className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-app-text border border-app-border hover:bg-app-raised rounded-md transition-colors"
          >
            <RefreshCw className="w-4 h-4" />
            {t('settings.updates.checkButton')}
          </button>
        )}

        {status === 'up-to-date' && (
          <div className="flex items-center gap-2 text-app-success text-sm">
            <span>✓</span>
            {t('settings.updates.upToDate', { version: appVersion })}
          </div>
        )}

        {status === 'checking' && (
          <div className="flex items-center gap-2 text-app-muted">
            <RefreshCw className="w-4 h-4 animate-spin" />
            {t('settings.updates.checking')}
          </div>
        )}

        {status === 'available' && updateInfo && (
          <div className="space-y-3">
            <div className="p-3 bg-app-accent-tint rounded-lg">
              <p className="text-sm font-medium text-blue-700 dark:text-app-accent">
                {t('settings.updates.available', { version: updateInfo.version })}
              </p>
              {updateInfo.body && (
                <p className="text-xs text-app-accent dark:text-blue-300 mt-1">
                  {updateInfo.body}
                </p>
              )}
            </div>
            <div className="flex gap-2">
              <button
                onClick={downloadAndInstall}
                className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-app-accent hover:bg-app-accent-hover rounded-md transition-colors"
              >
                <Download className="w-4 h-4" />
                {t('settings.updates.downloadInstall')}
              </button>
              <button
                onClick={dismiss}
                className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-app-text border border-app-border hover:bg-app-raised rounded-md transition-colors"
              >
                {t('common.later')}
              </button>
            </div>
          </div>
        )}

        {status === 'downloading' && (
          <div className="space-y-2">
            <div className="flex items-center gap-2 text-app-muted">
              <Download className="w-4 h-4 animate-pulse" />
              {t('settings.updates.downloading', { progress: Math.round(progress) })}
            </div>
            <div className="w-full bg-app-raised rounded-full h-2">
              <div
                className="bg-app-accent h-2 rounded-full transition-all duration-300"
                style={{ width: `${progress}%` }}
              />
            </div>
          </div>
        )}

        {status === 'ready' && (
          <div className="space-y-3">
            <p className="text-sm text-app-success">
              {t('settings.updates.ready')}
            </p>
            <button
              onClick={restartApp}
              className="px-4 py-2 text-sm font-medium text-white bg-green-600 hover:bg-green-700 rounded-md transition-colors"
            >
              {t('common.restartNow')}
            </button>
          </div>
        )}

        {status === 'error' && (
          <div className="space-y-2">
            <p className="text-sm text-app-danger">
              {error || t('settings.updates.errorDefault')}
            </p>
            <button
              onClick={checkForUpdates}
              className="text-sm text-app-accent hover:text-app-accent-hover dark:text-app-accent"
            >
              {t('common.tryAgain')}
            </button>
          </div>
        )}

        <p className="text-xs text-app-faint">
          {t('settings.updates.currentVersion', { version: appVersion })}
        </p>
      </div>
    </section>
  );
}

/**
 * Settings window component
 */
export function Settings() {
  const { t } = useTranslation();
  const {
    aiPolishEnabled,
    telemetryEnabled,
    shortcut,
    handsFreeMode,
    hidePillWhenInactive,
    historyEnabled,
    language,
    dictionary,
    history,
    loading,
    isPro,
    licenseKey,
    licenseStatus,
    licenseExpiresAt,
    licenseActivationCount,
    licenseActivationLimit,
    licenseLoading,
    licenseError,
    usage,
    loadSettings,
    saveSettings,
    resetSettings,
    loadDictionary,
    deleteEntry,
    clearDictionary,
    loadHistory,
    clearHistory,
    loadLicense,
    activateLicense,
    deactivateLicense,
    validateLicense,
    loadUsage,
  } = useSettingsStore();

  const isMac = navigator.platform.startsWith('Mac');

  const [appVersion, setAppVersion] = useState('...');
  const updateSectionRef = useRef<HTMLDivElement>(null);

  // Load dynamic app version
  useEffect(() => {
    getVersion().then(v => setAppVersion(v)).catch(() => {});
  }, []);

  // Keep the OS window title in sync with the current language. No deps array
  // intentionally — the call is cheap and ensures we don't desync after a
  // language switch (re-running on every render is acceptable here). Wrapped
  // for the ?preview= dev path where getCurrentWindow throws.
  useEffect(() => {
    try { getCurrentWindow().setTitle(t('windowTitle.settings')); }
    catch { /* not in Tauri (dev preview) */ }
  });

  // Scroll to update section when update-available event fires
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
  const [autostartOn, setAutostartOn] = useState(false);

  // Read the current OS-level autostart status on mount.
  useEffect(() => {
    isAutostartEnabled().then(setAutostartOn).catch(() => {});
  }, []);

  // Proactive Input Monitoring check whenever the saved shortcut is FnKey.
  // The Fn hotkey relies on a CGEventTap that silently no-ops when Input
  // Monitoring permission isn't granted — without this effect the user has
  // no in-app cue that anything is wrong until they explicitly re-click the
  // Fn radio. Now the warning + deep-link button show as soon as Settings
  // opens with a missing permission.
  useEffect(() => {
    if (shortcut !== 'FnKey') {
      // Clear any stale Input Monitoring warning if the user switched off Fn.
      if (
        shortcutError.includes('Input Monitoring') ||
        shortcutError === 'error.input_monitoring_required'
      ) {
        setShortcutError('');
      }
      return;
    }
    invoke<boolean>('check_input_monitoring')
      .then((hasPermission) => {
        if (!hasPermission) {
          setShortcutError('error.input_monitoring_required');
        } else if (
          shortcutError.includes('Input Monitoring') ||
          shortcutError === 'error.input_monitoring_required'
        ) {
          // Permission has been re-granted while Settings was open — clear.
          setShortcutError('');
        }
      })
      .catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shortcut]);

  const handleAutostartToggle = async (enabled: boolean) => {
    try {
      if (enabled) await enableAutostart(); else await disableAutostart();
      setAutostartOn(enabled);
      trackEvent("setting_changed", { setting_name: "autostart_enabled", new_value: String(enabled) });
    } catch (error) {
      console.error('Failed to update autostart:', error);
    }
  };

  // Check API key status
  const checkApiKeys = useCallback(() => {
    invoke<boolean>('has_groq_api_key').then(setHasGroqKey).catch(console.error);
  }, []);

  // Load settings, dictionary, and history on mount
  useEffect(() => {
    loadSettings();
    loadDictionary();
    loadHistory();
    loadLicense();
    loadUsage();
    checkApiKeys();
  }, [loadSettings, loadDictionary, loadHistory, loadLicense, loadUsage, checkApiKeys]);

  // React to license changes emitted by the backend (e.g. background re-validate)
  useTauriEvent('license-changed', () => {
    loadLicense();
    loadUsage();
  });

  // Refresh usage and history when a transcription completes.
  // Single listener intentionally — two competing useEffects on the same event
  // were the suspected source of the listener race in TTP-5.
  useTauriEvent<string>('recording-state-changed', (event) => {
    if (event.payload === 'Idle') {
      setTimeout(() => loadHistory(), 500);
      setTimeout(() => loadUsage(), 600);
    }
  });

  // Re-check API keys when window gets focus (e.g. after setup popup).
  // Window.onFocusChanged uses a different (synchronous) API so the legacy
  // pattern is OK here.
  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) checkApiKeys();
    });
    return () => { unlisten.then(fn => fn()); };
  }, [checkApiKeys]);

  // Refresh dictionary when backend auto-detects corrections
  useTauriEvent('dictionary-changed', () => {
    loadDictionary();
    loadUsage();
  });

  // Handle AI polish toggle
  const handlePolishToggle = async (enabled: boolean) => {
    try {
      await saveSettings({ ai_polish_enabled: enabled });
      trackEvent("setting_changed", { setting_name: "ai_polish_enabled", new_value: String(enabled) });
    } catch (error) {
      console.error('Failed to save AI polish setting:', error);
    }
  };

  // Handle telemetry toggle
  const handleTelemetryToggle = async () => {
    try {
      await saveSettings({ telemetry_enabled: !telemetryEnabled });
      trackEvent("setting_changed", { setting_name: "telemetry_enabled", new_value: String(!telemetryEnabled) });
      setShowRestartBanner(true);
    } catch (error) {
      console.error('Failed to save telemetry setting:', error);
    }
  };

  // Handle hands-free mode toggle
  const handleHandsFreeModeToggle = async (enabled: boolean) => {
    try {
      await saveSettings({ hands_free_mode: enabled });
      trackEvent("setting_changed", { setting_name: "hands_free_mode", new_value: String(enabled) });
    } catch (error) {
      console.error('Failed to save hands-free mode setting:', error);
    }
  };

  // Handle hide pill when inactive toggle
  const handleHidePillWhenInactiveToggle = async (enabled: boolean) => {
    try {
      await saveSettings({ hide_pill_when_inactive: enabled });
      trackEvent("setting_changed", { setting_name: "hide_pill_when_inactive", new_value: String(enabled) });
    } catch (error) {
      console.error('Failed to save hide pill when inactive setting:', error);
    }
  };

  // Handle history enabled toggle
  const handleHistoryEnabledToggle = async (enabled: boolean) => {
    try {
      await saveSettings({ history_enabled: enabled });
      trackEvent("setting_changed", { setting_name: "history_enabled", new_value: String(enabled) });
    } catch (error) {
      console.error('Failed to save history enabled setting:', error);
    }
  };

  // Handle Groq API key save (validates before saving)
  const handleGroqKeySave = async () => {
    if (!groqApiKey.trim()) return;
    setGroqKeySaving(true);
    setGroqKeyError('');
    setGroqKeySuccess(false);
    try {
      await invoke('validate_groq_api_key', { key: groqApiKey });
      await invoke('set_groq_api_key', { key: groqApiKey });
      setHasGroqKey(true);
      setGroqApiKey('');
      setGroqKeySuccess(true);
      setTimeout(() => setGroqKeySuccess(false), 3000);
    } catch (error) {
      setGroqKeyError(String(error));
    } finally {
      setGroqKeySaving(false);
    }
  };

  // Handle shortcut change from dropdown (includes Fn Key option)
  const handleShortcutChange = async (newShortcut: string) => {
    setShortcutError('');
    setShortcutSuccess(false);

    try {
      const isFnKey = newShortcut === 'FnKey';

      if (isFnKey) {
        // Request Input Monitoring permission (needed for Fn key detection)
        const hasPermission = await invoke<boolean>('check_input_monitoring');
        if (!hasPermission) {
          setShortcutError('error.input_monitoring_required');
          return;
        }
        // Unregister any existing global shortcut before enabling Fn mode
        try { await invoke('unregister_shortcuts_cmd'); } catch {}
        await invoke('set_fn_key_enabled', { enabled: true });
        await saveSettings({ shortcut: 'FnKey', fn_key_enabled: true });
        trackEvent("setting_changed", { setting_name: "shortcut", new_value: "FnKey" });
      } else {
        await invoke('set_fn_key_enabled', { enabled: false });
        await invoke('update_shortcut_cmd', { shortcut: newShortcut });
        await saveSettings({ shortcut: newShortcut, fn_key_enabled: false });
        trackEvent("setting_changed", { setting_name: "shortcut", new_value: newShortcut });
      }

      setShortcutSuccess(true);
      setTimeout(() => setShortcutSuccess(false), 3000);
    } catch (error) {
      console.error('Failed to update shortcut:', error);
      setShortcutError(String(error));
    }
  };

  // Handle clear dictionary
  const handleClearDictionary = async () => {
    try {
      await clearDictionary();
      setShowClearConfirm(false);
    } catch (error) {
      console.error('Failed to clear dictionary:', error);
    }
  };

  // Handle reset to defaults
  const handleResetDefaults = async () => {
    try {
      await resetSettings();
      await clearDictionary();
      setShowResetConfirm(false);
    } catch (error) {
      console.error('Failed to reset settings:', error);
    }
  };

  // Handle add dictionary entry
  const handleAddEntry = async () => {
    setAddEntryError('');
    const orig = newOriginal.trim();
    const corr = newCorrection.trim();
    if (!orig || !corr) {
      setAddEntryError(t('settings.dictionary.bothRequired'));
      return;
    }
    if (orig === corr) {
      setAddEntryError(t('settings.dictionary.mustBeDifferent'));
      return;
    }
    try {
      await invoke('add_dictionary_entry', { original: orig, correction: corr });
      setNewOriginal('');
      setNewCorrection('');
      await loadDictionary();
      await loadUsage();
    } catch (error) {
      setAddEntryError(String(error));
    }
  };

  // Handle delete single entry — stable ref so DictionaryRow's React.memo holds.
  const handleDeleteEntry = useCallback(async (original: string) => {
    try {
      await deleteEntry(original);
    } catch (error) {
      console.error('Failed to delete entry:', error);
    }
  }, [deleteEntry]);

  // Handle clear history
  const handleClearHistory = async () => {
    try {
      await clearHistory();
      setShowClearHistoryConfirm(false);
    } catch (error) {
      console.error('Failed to clear history:', error);
    }
  };

  // Handle license activation
  const handleActivateLicense = async () => {
    const key = licenseInput.trim();
    if (!key) return;
    try {
      await activateLicense(key);
      setLicenseInput('');
      trackEvent('license_activated', {});
    } catch (error) {
      console.error('Activation failed:', error);
    }
  };

  // Handle license deactivation
  const handleDeactivateLicense = async () => {
    try {
      await deactivateLicense();
      setShowDeactivateConfirm(false);
      trackEvent('license_deactivated', {});
    } catch (error) {
      console.error('Deactivation failed:', error);
    }
  };

  // Format expiration timestamp into a readable date
  const formatExpiry = (ts: number | null): string => {
    if (!ts) return t('settings.pro.neverExpires');
    const date = new Date(ts * 1000);
    return t('settings.pro.expiresOn', { date: date.toLocaleDateString() });
  };

  // Mask the license key for display (show first/last 4 chars)
  const maskedLicenseKey = licenseKey
    ? `${licenseKey.slice(0, 4)}…${licenseKey.slice(-4)}`
    : '';

  const isInTrial = !!usage?.is_in_trial && !isPro;
  const trialDaysLeft = usage?.trial_days_left ?? 0;
  const polishUsed = usage?.polish_count_this_month ?? 0;
  const polishLimit = usage?.polish_limit_free ?? 0;
  const dictCount = usage?.dictionary_count ?? dictionary.length;
  const dictLimit = usage?.dictionary_limit_free ?? 20;
  const histCount = usage?.history_count ?? history.length;
  const histLimit = usage?.history_limit_free ?? 50;
  const dictAtCap = !isPro && !isInTrial && dictCount >= dictLimit;
  const histAtCap = !isPro && !isInTrial && histCount >= histLimit;
  const polishAtCap = !isPro && !isInTrial && polishUsed >= polishLimit;

  // Build the recording-trigger options at render time so labels and descriptions
  // re-translate when the language changes. Keeping the structure as plain
  // objects (rather than translating inside the JSX) keeps the .map() loop
  // small and easy to follow.
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

  // Translate keys; pass raw human-readable strings through. Used for any error
  // surfaced from Rust commands or internal flags — Rust now emits translation
  // keys like 'error.license_key_empty', but legacy paths may still return raw
  // strings.
  const translateIfKey = (s: string | null): string =>
    s && (s.startsWith('error.') || s.startsWith('permission.')) ? t(s) : (s ?? '');
  const showShortcutError = translateIfKey(shortcutError);
  const showLicenseError = translateIfKey(licenseError);
  const shortcutErrorIsInputMonitoring =
    shortcutError.includes('Input Monitoring') ||
    shortcutError === 'error.input_monitoring_required';

  return (
    <div className="min-h-screen flex bg-app-bg text-app-text bg-noise">
      <SettingsSidebar />
      <main className="flex-1 min-w-0 overflow-y-auto">
        <div className="max-w-2xl mx-auto px-8 pt-8 pb-12">
          {/* Permission warning sits above everything else — silent permission
              loss (esp. Accessibility after an update) was the most-reported
              class of "TTP isn't working" issues. */}
          <PermissionBanner />

          {/* Your usage — pinned at the top so the first thing the user sees
              in Settings is their own activity, not the About hero. */}
          <div id="usage" data-section="usage" className="scroll-mt-6">
          <AnalyticsSection />
          </div>

        {/* Welcome / About — flat token-driven card (the old radial gradient
            clashed against the rest of the surface chrome). */}
        <section id="account" data-section="account" className="scroll-mt-6 bg-app-surface border border-app-border rounded-app-lg shine-sm p-6 mb-6">
          <h1 className="text-xl font-semibold tracking-tight mb-1 text-app-text">TTP by AmirKS</h1>
          <p className="text-app-accent text-xs font-medium mb-3">{t('settings.about.subtitle', { version: appVersion })}</p>
          <p className="text-sm text-app-muted leading-relaxed mb-3">
            {t('settings.about.description')}
          </p>
          <p className="text-sm text-app-muted leading-relaxed mb-3">
            {t('settings.about.author')}
          </p>
          <div className="p-3 bg-app-raised border border-app-border rounded-app-sm mb-4">
            <p className="text-xs text-app-muted leading-relaxed">
              <span className="text-app-success font-medium">{t('settings.about.privacyLabel')}</span>{' '}
              {t('settings.about.privacyBody')}
            </p>
          </div>
          <div className="flex gap-3">
            <a
              href="https://amirks.eu"
              target="_blank"
              rel="noopener noreferrer"
              className="px-3 py-1.5 text-xs font-medium bg-app-accent hover:bg-app-accent-hover text-app-accent-fg rounded-app-sm transition-colors"
            >
              amirks.eu
            </a>
            <a
              href="https://www.linkedin.com/in/amirks/"
              target="_blank"
              rel="noopener noreferrer"
              className="px-3 py-1.5 text-xs font-medium bg-app-raised hover:bg-app-border text-app-text rounded-app-sm transition-colors"
            >
              LinkedIn
            </a>
          </div>
        </section>

        {/* TTP Pro Section */}
        <section id="pro" data-section="pro" className="scroll-mt-6 bg-app-surface rounded-app-lg shine-sm border border-app-border p-6 mb-6">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-2">
              <Crown className={`w-5 h-5 ${isPro || isInTrial ? 'text-amber-500' : 'text-app-faint'}`} />
              <h2 className="text-lg font-semibold text-app-text">
                {t('settings.pro.title')}
              </h2>
              {isPro && (
                <span className="px-2 py-0.5 text-xs font-semibold rounded-full bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-300">
                  {t('settings.pro.badgeActive')}
                </span>
              )}
              {!isPro && isInTrial && (
                <span className="px-2 py-0.5 text-xs font-semibold rounded-full bg-app-accent-tint text-app-accent">
                  {t('settings.pro.badgeTrial', { days: trialDaysLeft })}
                </span>
              )}
            </div>
            {isPro && (
              <button
                onClick={validateLicense}
                disabled={licenseLoading}
                className="text-xs text-app-muted hover:text-app-text flex items-center gap-1 disabled:opacity-50"
                title={t('settings.pro.refreshTitle')}
              >
                {licenseLoading ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <RefreshCw className="w-3.5 h-3.5" />
                )}
                {t('common.refresh')}
              </button>
            )}
          </div>

          {!isPro ? (
            <>
              <p className="text-sm text-app-muted mb-4">
                {isInTrial
                  ? t('settings.pro.descTrial', { days: trialDaysLeft })
                  : t('settings.pro.descFree')}
              </p>

              {/* Free tier usage counters */}
              {!isInTrial && (
                <div className="space-y-2 mb-4 p-3 bg-app-bg/40 rounded-md">
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-app-muted">{t('settings.pro.usagePolish')}</span>
                    <span className={`font-mono ${polishAtCap ? 'text-app-danger font-semibold' : 'text-app-text'}`}>
                      {polishUsed} / {polishLimit}
                    </span>
                  </div>
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-app-muted">{t('settings.pro.usageDictionary')}</span>
                    <span className={`font-mono ${dictAtCap ? 'text-app-danger font-semibold' : 'text-app-text'}`}>
                      {dictCount} / {dictLimit}
                    </span>
                  </div>
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-app-muted">{t('settings.pro.usageHistory')}</span>
                    <span className={`font-mono ${histAtCap ? 'text-app-danger font-semibold' : 'text-app-text'}`}>
                      {histCount} / {histLimit}
                    </span>
                  </div>
                </div>
              )}

              <div className="space-y-2 mb-4">
                <input
                  type="text"
                  value={licenseInput}
                  onChange={(e) => setLicenseInput(e.target.value)}
                  placeholder={t('settings.pro.inputPlaceholder')}
                  spellCheck={false}
                  className="w-full px-3 py-2 text-sm font-mono border border-app-border rounded-md bg-app-surface text-app-text focus:outline-none focus:ring-2 focus:ring-app-accent"
                  disabled={licenseLoading}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && licenseInput.trim()) {
                      handleActivateLicense();
                    }
                  }}
                />
                {licenseError && (
                  <p className="text-xs text-app-danger">{showLicenseError}</p>
                )}
              </div>
              <div className="flex items-center gap-3">
                <button
                  onClick={handleActivateLicense}
                  disabled={licenseLoading || !licenseInput.trim()}
                  className="px-4 py-2 text-sm font-medium text-white bg-app-accent hover:bg-app-accent-hover disabled:opacity-50 disabled:cursor-not-allowed rounded-md transition-colors flex items-center gap-2"
                >
                  {licenseLoading && <Loader2 className="w-4 h-4 animate-spin" />}
                  {t('settings.pro.activate')}
                </button>
                <a
                  href="https://amirks.lemonsqueezy.com/buy/dcc74241-21ae-4d20-8a3c-90bf8d842bae"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-sm font-medium text-app-accent hover:text-app-accent-hover dark:text-app-accent dark:hover:text-blue-300"
                >
                  {t('settings.pro.buyLink')}
                </a>
              </div>
            </>
          ) : (
            <>
              <div className="space-y-2 mb-4">
                <div className="flex items-center justify-between text-sm">
                  <span className="text-app-muted">{t('settings.pro.labelKey')}</span>
                  <span className="font-mono text-app-text">{maskedLicenseKey}</span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-app-muted">{t('settings.pro.labelStatus')}</span>
                  <span className="text-app-text capitalize">
                    {licenseStatus ?? t('settings.pro.statusUnknown')}
                  </span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-app-muted">{t('settings.pro.labelValidity')}</span>
                  <span className="text-app-text">
                    {formatExpiry(licenseExpiresAt)}
                  </span>
                </div>
                {licenseActivationLimit !== null && (
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-app-muted">{t('settings.pro.labelActivations')}</span>
                    <span className="text-app-text">
                      {licenseActivationCount ?? 0} / {licenseActivationLimit}
                    </span>
                  </div>
                )}
              </div>
              {licenseError && (
                <p className="text-xs text-app-danger mb-3">{showLicenseError}</p>
              )}
              <button
                onClick={() => setShowDeactivateConfirm(true)}
                disabled={licenseLoading}
                className="text-sm font-medium text-red-600 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300 disabled:opacity-50"
              >
                {t('settings.pro.deactivateDevice')}
              </button>
              <p className="text-xs text-app-muted mt-2">
                {t('settings.pro.deactivateHelp')}
              </p>
            </>
          )}
        </section>

        {/* Recording Trigger Section */}
        <section id="recording" data-section="recording" className="scroll-mt-6 bg-app-surface rounded-app-lg shine-sm border border-app-border p-6 mb-6">
          <h2 className="text-lg font-semibold text-app-text mb-4">
            {t('settings.recordingTrigger.title')}
          </h2>
          <p className="text-sm text-app-muted mb-4">
            {t('settings.recordingTrigger.desc')}
          </p>

          <div className="space-y-2">
            {triggerOptions.map((opt) => (
              <button
                key={opt.value}
                onClick={() => handleShortcutChange(opt.value)}
                disabled={loading}
                className={`
                  w-full flex items-center justify-between px-4 py-3 rounded-lg border-2 transition-all text-left
                  ${shortcut === opt.value
                    ? 'border-app-accent bg-app-accent-tint'
                    : 'border-app-border hover:border-app-border-strong'
                  }
                `}
              >
                <div className="flex items-center gap-3">
                  <span className={`
                    w-4 h-4 rounded-full border-2 flex items-center justify-center flex-shrink-0
                    ${shortcut === opt.value
                      ? 'border-app-accent'
                      : 'border-app-border'
                    }
                  `}>
                    {shortcut === opt.value && (
                      <span className="w-2 h-2 rounded-full bg-app-accent" />
                    )}
                  </span>
                  <span className={`font-mono text-sm font-semibold ${
                    shortcut === opt.value
                      ? 'text-app-accent'
                      : 'text-app-text'
                  }`}>
                    {opt.label}
                  </span>
                </div>
                {opt.desc && (
                  <span className={`text-xs ${
                    opt.recommended
                      ? 'text-app-accent dark:text-app-accent font-medium'
                      : 'text-app-faint'
                  }`}>
                    {opt.desc}
                  </span>
                )}
              </button>
            ))}
          </div>

          {shortcutError && (
            <div className="mt-3 space-y-2">
              <p className="text-sm text-app-danger">
                {showShortcutError}
              </p>
              {shortcutErrorIsInputMonitoring && (
                <button
                  type="button"
                  onClick={() => {
                    invoke('open_input_monitoring_settings').catch((err) => {
                      console.error('open_input_monitoring_settings failed', err);
                    });
                  }}
                  className="inline-flex items-center gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-3 py-1.5 text-xs font-medium text-red-600 dark:text-red-300 hover:bg-red-500/20 transition-colors"
                >
                  {t('settings.recordingTrigger.openInputMonitoring')}
                </button>
              )}
            </div>
          )}

          {shortcutSuccess && (
            <p className="text-sm text-app-success mt-3">
              {t('settings.recordingTrigger.successUpdated')}
            </p>
          )}
        </section>

        {/* Recording Mode Section */}
        <section className="bg-app-surface rounded-app-lg shine-sm border border-app-border p-6 mb-6">
          <h2 className="text-lg font-semibold text-app-text mb-4">
            {t('settings.recordingMode.title')}
          </h2>

          {/* Hands-free mode toggle */}
          <div className="flex items-center justify-between mb-4">
            <div className="flex-1 pr-4">
              <p className="text-app-text font-medium">
                {t('settings.recordingMode.handsFreeLabel')}
              </p>
              <p className="text-sm text-app-muted mt-1">
                {t('settings.recordingMode.handsFreeDesc')}
              </p>
            </div>
            <Toggle
              enabled={handsFreeMode}
              onChange={handleHandsFreeModeToggle}
              disabled={loading}
            />
          </div>

          {/* Hide pill when inactive toggle */}
          <div className="flex items-center justify-between mb-4">
            <div className="flex-1 pr-4">
              <p className="text-app-text font-medium">
                {t('settings.recordingMode.hidePillLabel')}
              </p>
              <p className="text-sm text-app-muted mt-1">
                {t('settings.recordingMode.hidePillDesc')}
              </p>
            </div>
            <Toggle
              enabled={hidePillWhenInactive}
              onChange={handleHidePillWhenInactiveToggle}
              disabled={loading}
            />
          </div>

          {/* Launch at startup toggle */}
          <div className="flex items-center justify-between">
            <div className="flex-1 pr-4">
              <p className="text-app-text font-medium">
                {t('settings.recordingMode.launchStartupLabel')}
              </p>
              <p className="text-sm text-app-muted mt-1">
                {t('settings.recordingMode.launchStartupDesc')}
              </p>
            </div>
            <Toggle
              enabled={autostartOn}
              onChange={handleAutostartToggle}
              disabled={loading}
            />
          </div>
        </section>

        {/* Transcription Section */}
        <section id="transcription" data-section="transcription" className="scroll-mt-6 bg-app-surface rounded-app-lg shine-sm border border-app-border p-6 mb-6">
          <h2 className="text-lg font-semibold text-app-text mb-4">
            {t('settings.transcription.title')}
          </h2>

          {/* Groq API Key */}
          <div className="mb-6 p-4 bg-app-raised rounded-lg">
            <p className="text-app-text font-medium mb-2">
              {t('settings.transcription.groqLabel')}
            </p>
              {hasGroqKey ? (
                <div className="flex items-center gap-2">
                  <span className="text-sm text-app-success">{t('settings.transcription.keyConfigured')}</span>
                  <button
                    onClick={() => setHasGroqKey(false)}
                    className="text-sm text-app-muted hover:text-app-text"
                  >
                    {t('common.change')}
                  </button>
                </div>
              ) : (
                <div className="space-y-2">
                  <div className="flex gap-2">
                    <input
                      type="password"
                      value={groqApiKey}
                      onChange={(e) => setGroqApiKey(e.target.value)}
                      placeholder={t('settings.transcription.keyPlaceholder')}
                      className="flex-1 px-3 py-2 border border-app-border rounded-md bg-app-surface text-app-text focus:outline-none focus:ring-2 focus:ring-app-accent text-sm"
                    />
                    <button
                      onClick={handleGroqKeySave}
                      disabled={groqKeySaving || !groqApiKey.trim()}
                      className="px-4 py-2 text-sm font-medium text-white bg-app-accent hover:bg-app-accent-hover disabled:bg-app-raised disabled:cursor-not-allowed rounded-md transition-colors"
                    >
                      {groqKeySaving ? t('common.validating') : t('common.save')}
                    </button>
                  </div>
                  {groqKeySuccess && (
                    <p className="text-sm text-app-success">
                      {t('settings.transcription.keySaved')}
                    </p>
                  )}
                  {groqKeyError && (
                    <p className="text-sm text-app-danger">
                      {groqKeyError}
                    </p>
                  )}
                  <p className="text-xs text-app-muted">
                    {t('settings.transcription.getKeyAt')}{' '}
                    <a
                      href="https://console.groq.com/keys"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-app-accent hover:underline"
                    >
                      console.groq.com
                    </a>
                  </p>
                </div>
              )}
          </div>

          {/* AI Polish Toggle */}
          <div className="flex items-center justify-between">
            <div className="flex-1 pr-4">
              <p className="text-app-text font-medium">
                {t('settings.transcription.polishLabel')}
              </p>
              <p className="text-sm text-app-muted mt-1">
                {t('settings.transcription.polishDesc')}
              </p>
            </div>
            <Toggle
              enabled={aiPolishEnabled}
              onChange={handlePolishToggle}
              disabled={loading}
            />
          </div>

        </section>

        {/* Privacy & Telemetry Section */}
        <section id="privacy" data-section="privacy" className="scroll-mt-6 bg-app-surface rounded-app-lg shine-sm border border-app-border p-6 mb-6">
          <h2 className="text-lg font-semibold text-app-text mb-4">
            {t('settings.privacy.title')}
          </h2>

          <div className="flex items-center justify-between mb-4">
            <div className="flex-1 pr-4">
              <p className="text-app-text font-medium">
                {t('settings.privacy.helpLabel')}
              </p>
              <p className="text-sm text-app-muted mt-1">
                {t('settings.privacy.helpDesc')}
              </p>
            </div>
            <Toggle
              enabled={telemetryEnabled}
              onChange={handleTelemetryToggle}
              disabled={loading}
            />
          </div>

          {/* Restart banner -- shown after toggling */}
          {showRestartBanner && (
            <div className="flex items-center justify-between p-3 bg-app-warning-tint rounded-lg mb-4">
              <p className="text-sm text-amber-700 dark:text-amber-400">
                {t('settings.privacy.restartHint')}
              </p>
              <button
                onClick={() => relaunch().catch(console.error)}
                className="px-3 py-1.5 text-xs font-medium text-amber-700 dark:text-amber-400 border border-amber-300 dark:border-amber-700 hover:bg-amber-100 dark:hover:bg-amber-900/40 rounded-md transition-colors ml-3 whitespace-nowrap"
              >
                {t('common.restartNow')}
              </button>
            </div>
          )}

          {/* Privacy explanation */}
          <div className="p-3 bg-app-raised rounded-lg">
            <p className="text-xs text-app-muted leading-relaxed">
              <span className="font-medium text-app-text">{t('settings.privacy.whatSentLabel')}</span>{' '}
              {t('settings.privacy.whatSentBody')}
            </p>
            <p className="text-xs text-app-muted leading-relaxed mt-2">
              <span className="font-medium text-app-text">{t('settings.privacy.neverSentLabel')}</span>{' '}
              {t('settings.privacy.neverSentBody')}
            </p>
          </div>
        </section>

        {/* Dictionary Section */}
        <section id="dictionary" data-section="dictionary" className="scroll-mt-6 bg-app-surface rounded-app-lg shine-sm border border-app-border p-6 mb-6">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-lg font-semibold text-app-text">
              {t('settings.dictionary.title')}
            </h2>
            {dictionary.length > 0 && (
              <button
                onClick={() => setShowClearConfirm(true)}
                className="text-sm text-red-600 hover:text-red-700 font-medium"
              >
                {t('common.clearAll')}
              </button>
            )}
          </div>

          {/* Add entry form */}
          <div className="mb-4 flex gap-2 items-end">
            <div className="flex-1">
              <label className="block text-xs text-app-muted mb-1">{t('settings.dictionary.labelMisheard')}</label>
              <input
                type="text"
                value={newOriginal}
                onChange={(e) => setNewOriginal(e.target.value)}
                placeholder={t('settings.dictionary.placeholderMisheard')}
                disabled={dictAtCap}
                className="w-full px-3 py-1.5 border border-app-border rounded-md bg-app-surface text-app-text text-sm focus:outline-none focus:ring-2 focus:ring-app-accent disabled:opacity-50"
              />
            </div>
            <span className="text-app-faint pb-1.5">&rarr;</span>
            <div className="flex-1">
              <label className="block text-xs text-app-muted mb-1">{t('settings.dictionary.labelCorrection')}</label>
              <input
                type="text"
                value={newCorrection}
                onChange={(e) => setNewCorrection(e.target.value)}
                placeholder={t('settings.dictionary.placeholderCorrection')}
                disabled={dictAtCap}
                className="w-full px-3 py-1.5 border border-app-border rounded-md bg-app-surface text-app-text text-sm focus:outline-none focus:ring-2 focus:ring-app-accent disabled:opacity-50"
              />
            </div>
            <button
              onClick={handleAddEntry}
              disabled={!newOriginal.trim() || !newCorrection.trim() || dictAtCap}
              className="px-3 py-1.5 text-sm font-medium text-white bg-app-accent hover:bg-app-accent-hover disabled:bg-app-raised disabled:cursor-not-allowed rounded-md transition-colors"
            >
              {t('common.add')}
            </button>
          </div>
          {dictAtCap && (
            <p className="text-amber-600 dark:text-amber-400 text-xs mb-3">
              {t('settings.dictionary.limitReached', { limit: dictLimit })}
            </p>
          )}
          {addEntryError && (
            <p className="text-red-500 text-sm mb-3">{addEntryError}</p>
          )}

          {dictionary.length === 0 ? (
            <p className="text-app-muted text-center py-4">
              {t('settings.dictionary.emptyState')}
            </p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b border-app-border">
                    <th className="text-left py-2 px-4 text-xs font-medium text-app-muted uppercase tracking-wider">
                      {t('settings.dictionary.tableOriginal')}
                    </th>
                    <th className="text-left py-2 px-4 text-xs font-medium text-app-muted uppercase tracking-wider">
                      {t('settings.dictionary.tableCorrection')}
                    </th>
                    <th className="w-20"></th>
                  </tr>
                </thead>
                <tbody>
                  {dictionary.map((entry) => (
                    <DictionaryRow
                      key={entry.original}
                      entry={entry}
                      onDelete={handleDeleteEntry}
                    />
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>

        {/* History Section */}
        <section id="history" data-section="history" className="scroll-mt-6 bg-app-surface rounded-app-lg shine-sm border border-app-border p-6 mb-6">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-lg font-semibold text-app-text">
              {t('settings.history.title')}
            </h2>
            {history.length > 0 && (
              <button
                onClick={() => setShowClearHistoryConfirm(true)}
                className="text-sm text-red-600 hover:text-red-700 font-medium"
              >
                {t('settings.history.clearButton')}
              </button>
            )}
          </div>

          {/* Save history toggle */}
          <div className="flex items-center justify-between mb-4 pb-4 border-b border-app-border">
            <div className="flex-1 pr-4">
              <p className="text-app-text font-medium">
                {t('settings.history.saveLabel')}
              </p>
              <p className="text-sm text-app-muted mt-1">
                {t('settings.history.saveDesc')}
              </p>
            </div>
            <Toggle
              enabled={historyEnabled}
              onChange={handleHistoryEnabledToggle}
              disabled={loading}
            />
          </div>

          {history.length === 0 ? (
            <p className="text-app-muted text-center py-8">
              {historyEnabled ? t('settings.history.emptyEnabled') : t('settings.history.emptyDisabled')}
            </p>
          ) : (
            <div className="max-h-80 overflow-y-auto rounded-md border border-app-border">
              {history.map((entry, index) => (
                <HistoryRow key={`${entry.timestamp}-${index}`} entry={entry} />
              ))}
            </div>
          )}
        </section>

        {/* Update Channel Section — beta opt-in */}
        <div id="updates" data-section="updates" className="scroll-mt-6">
        <UpdateChannelSection />

        {/* Updates Section */}
        <div ref={updateSectionRef}>
          <UpdateSection />
        </div>

        </div>

        {/* Language Section — UI/tray/notification locale.
            'system' resolves from navigator.language at runtime (fr-* → fr, else en).
            Saving the choice emits 'settings-changed' which the main.tsx listener
            picks up to call i18n.changeLanguage across every open window. */}
        <section id="language" data-section="language" className="scroll-mt-6 bg-app-surface rounded-app-lg shine-sm border border-app-border p-6 mb-6">
          <h2 className="text-lg font-semibold text-app-text mb-2">
            {t('settings.language.title')}
          </h2>
          <p className="text-sm text-app-muted mb-4">
            {t('settings.language.desc')}
          </p>
          <div className="space-y-2">
            {([
              { value: 'system' as LanguageChoice, label: t('settings.language.optionSystem') },
              { value: 'en' as LanguageChoice, label: t('settings.language.optionEnglish') },
              { value: 'fr' as LanguageChoice, label: t('settings.language.optionFrench') },
            ]).map((opt) => (
              <button
                key={opt.value}
                onClick={async () => {
                  if (language === opt.value) return;
                  try {
                    await saveSettings({ language: opt.value });
                    trackEvent('setting_changed', {
                      setting_name: 'language',
                      new_value: opt.value,
                    });
                  } catch (error) {
                    console.error('Failed to save language setting:', error);
                  }
                }}
                disabled={loading}
                className={`
                  w-full flex items-center px-4 py-3 rounded-lg border-2 transition-all text-left
                  ${language === opt.value
                    ? 'border-app-accent bg-app-accent-tint'
                    : 'border-app-border hover:border-app-border-strong'
                  }
                `}
              >
                <span className={`
                  w-4 h-4 rounded-full border-2 flex items-center justify-center flex-shrink-0 mr-3
                  ${language === opt.value
                    ? 'border-app-accent'
                    : 'border-app-border'
                  }
                `}>
                  {language === opt.value && (
                    <span className="w-2 h-2 rounded-full bg-app-accent" />
                  )}
                </span>
                <span className={`text-sm font-medium ${
                  language === opt.value
                    ? 'text-app-accent'
                    : 'text-app-text'
                }`}>
                  {opt.label}
                </span>
              </button>
            ))}
          </div>
        </section>

        {/* Reset Section */}
        <section id="advanced" data-section="advanced" className="scroll-mt-6 bg-app-surface rounded-app-lg shine-sm border border-app-border p-6">
          <h2 className="text-lg font-semibold text-app-text mb-4">
            {t('settings.reset.title')}
          </h2>
          <p className="text-sm text-app-muted mb-4">
            {t('settings.reset.desc')}
          </p>
          <button
            onClick={() => setShowResetConfirm(true)}
            className="px-4 py-2 text-sm font-medium text-red-600 border border-red-600 hover:bg-app-danger-tint rounded-md transition-colors"
          >
            {t('settings.reset.button')}
          </button>
        </section>

        {/* Confirmation Dialogs */}
        <ConfirmDialog
          open={showClearConfirm}
          title={t('dialog.clearDictionary.title')}
          message={t('dialog.clearDictionary.message')}
          confirmText={t('dialog.clearDictionary.confirm')}
          onConfirm={handleClearDictionary}
          onCancel={() => setShowClearConfirm(false)}
        />

        <ConfirmDialog
          open={showResetConfirm}
          title={t('dialog.reset.title')}
          message={t('dialog.reset.message')}
          confirmText={t('dialog.reset.confirm')}
          onConfirm={handleResetDefaults}
          onCancel={() => setShowResetConfirm(false)}
        />

        <ConfirmDialog
          open={showClearHistoryConfirm}
          title={t('dialog.clearHistory.title')}
          message={t('dialog.clearHistory.message')}
          confirmText={t('dialog.clearHistory.confirm')}
          onConfirm={handleClearHistory}
          onCancel={() => setShowClearHistoryConfirm(false)}
        />

        <ConfirmDialog
          open={showDeactivateConfirm}
          title={t('dialog.deactivateLicense.title')}
          message={t('dialog.deactivateLicense.message')}
          confirmText={t('dialog.deactivateLicense.confirm')}
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
   SettingsSidebar — left nav rail. Compact icon+label list, sticks to viewport.
   Click scrolls the matching section into view; active state tracks via
   IntersectionObserver so scroll position highlights the right row.
   ------------------------------------------------------------------------- */

interface SidebarItem {
  id: string;
  labelKey: string;
  icon: typeof Crown;
}

function SettingsSidebar() {
  const { t } = useTranslation();
  const [appVersion, setAppVersion] = useState('');
  const [active, setActive] = useState('usage');

  useEffect(() => {
    getVersion().then((v) => setAppVersion(v)).catch(() => {});
  }, []);

  // Track which section is currently in view. The rootMargin biases the
  // detection to the upper third — feels right because users typically
  // scroll a section into the top half before scanning down.
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
    targets.forEach((t) => observer.observe(t));
    return () => observer.disconnect();
  }, []);

  const items: SidebarItem[] = [
    { id: 'usage', labelKey: 'settings.nav.usage', icon: Activity },
    { id: 'account', labelKey: 'settings.nav.account', icon: User },
    { id: 'pro', labelKey: 'settings.nav.pro', icon: Crown },
    { id: 'recording', labelKey: 'settings.nav.recording', icon: Mic },
    { id: 'transcription', labelKey: 'settings.nav.transcription', icon: Languages },
    { id: 'dictionary', labelKey: 'settings.nav.dictionary', icon: BookOpen },
    { id: 'history', labelKey: 'settings.nav.history', icon: Clock },
    { id: 'updates', labelKey: 'settings.nav.updates', icon: Download },
    { id: 'language', labelKey: 'settings.nav.language', icon: Globe },
    { id: 'advanced', labelKey: 'settings.nav.advanced', icon: SlidersHorizontal },
  ];

  const onSelect = (id: string) => {
    const el = document.getElementById(id);
    if (el) {
      el.scrollIntoView({ behavior: 'smooth', block: 'start' });
      setActive(id);
    }
  };

  return (
    <aside className="w-56 shrink-0 bg-app-dim border-r border-app-border flex flex-col h-screen sticky top-0">
      <div className="px-5 pt-6 pb-4">
        <div className="flex items-center gap-2.5">
          <div className="relative size-7 rounded-app-sm bg-app-surface border border-app-border grid place-items-center shine-sm">
            <span className="text-app-text font-semibold text-[10px] tracking-[-0.02em]">TTP</span>
            <span
              aria-hidden
              className="absolute top-[5px] right-[5px] size-[3px] rounded-full bg-app-accent"
            />
          </div>
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
                    'transition-colors duration-100',
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
    </aside>
  );
}

export default Settings;
