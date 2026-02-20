// TTP - Talk To Paste
// Settings window - configure app behavior and manage dictionary

import { useEffect, useState, useCallback, useRef } from 'react';
import { Copy, Check, Download, RefreshCw } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getVersion } from '@tauri-apps/api/app';
import { relaunch } from '@tauri-apps/plugin-process';
import { trackEvent } from '../lib/analytics';
import { useUpdater } from '../hooks/useUpdater';
import { useSettingsStore, DictionaryEntry, HistoryEntry } from '../stores/settings-store';

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
        transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2
        ${enabled ? 'bg-blue-600' : 'bg-gray-200 dark:bg-gray-700'}
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
  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-6 max-w-sm mx-4">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-2">
          {title}
        </h3>
        <p className="text-gray-600 dark:text-gray-400 mb-4">{message}</p>
        <div className="flex justify-end gap-3">
          <button
            onClick={onCancel}
            className="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-md transition-colors"
          >
            Cancel
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
 * Dictionary table row component
 */
function DictionaryRow({
  entry,
  onDelete,
}: {
  entry: DictionaryEntry;
  onDelete: () => void;
}) {
  return (
    <tr className="border-b border-gray-200 dark:border-gray-700">
      <td className="py-3 px-4 text-gray-900 dark:text-white font-mono text-sm">
        {entry.original}
      </td>
      <td className="py-3 px-4 text-gray-900 dark:text-white font-mono text-sm">
        {entry.correction}
      </td>
      <td className="py-3 px-4 text-right">
        <button
          onClick={onDelete}
          className="text-red-600 hover:text-red-700 text-sm font-medium"
        >
          Delete
        </button>
      </td>
    </tr>
  );
}

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
function HistoryRow({ entry }: { entry: HistoryEntry }) {
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
    <div className="flex items-start gap-3 p-3 odd:bg-gray-50 dark:odd:bg-gray-800/50">
      <div className="flex-1 min-w-0">
        <p className="text-xs text-gray-500 dark:text-gray-400 mb-1">
          {formatTimestamp(entry.timestamp)}
        </p>
        <p className="text-sm text-gray-900 dark:text-white break-words">
          {preview}
        </p>
      </div>
      <button
        onClick={handleCopy}
        className="flex-shrink-0 p-2 text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 rounded-md hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
        title="Copy to clipboard"
      >
        {copied ? (
          <Check className="w-4 h-4 text-green-500" />
        ) : (
          <Copy className="w-4 h-4" />
        )}
      </button>
    </div>
  );
}

/**
 * Update section component
 */
function UpdateSection() {
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
  useEffect(() => {
    const unlisten = listen('update-available', () => {
      checkForUpdates();
    });
    return () => { unlisten.then(fn => fn()); };
  }, [checkForUpdates]);

  return (
    <section className="bg-white dark:bg-gray-800 rounded-lg shadow-sm p-6 mb-6">
      <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
        Updates
      </h2>

      <div className="space-y-3">
        {status === 'idle' && (
          <button
            onClick={checkForUpdates}
            className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 border border-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700 rounded-md transition-colors"
          >
            <RefreshCw className="w-4 h-4" />
            Check for Updates
          </button>
        )}

        {status === 'checking' && (
          <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400">
            <RefreshCw className="w-4 h-4 animate-spin" />
            Checking for updates...
          </div>
        )}

        {status === 'available' && updateInfo && (
          <div className="space-y-3">
            <div className="p-3 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
              <p className="text-sm font-medium text-blue-700 dark:text-blue-400">
                Update available: v{updateInfo.version}
              </p>
              {updateInfo.body && (
                <p className="text-xs text-blue-600 dark:text-blue-300 mt-1">
                  {updateInfo.body}
                </p>
              )}
            </div>
            <div className="flex gap-2">
              <button
                onClick={downloadAndInstall}
                className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-md transition-colors"
              >
                <Download className="w-4 h-4" />
                Download and Install
              </button>
              <button
                onClick={dismiss}
                className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 border border-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700 rounded-md transition-colors"
              >
                Later
              </button>
            </div>
          </div>
        )}

        {status === 'downloading' && (
          <div className="space-y-2">
            <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400">
              <Download className="w-4 h-4 animate-pulse" />
              Downloading... {Math.round(progress)}%
            </div>
            <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
              <div
                className="bg-blue-600 h-2 rounded-full transition-all duration-300"
                style={{ width: `${progress}%` }}
              />
            </div>
          </div>
        )}

        {status === 'ready' && (
          <div className="space-y-3">
            <p className="text-sm text-green-600 dark:text-green-400">
              Update downloaded! Restart to apply.
            </p>
            <button
              onClick={restartApp}
              className="px-4 py-2 text-sm font-medium text-white bg-green-600 hover:bg-green-700 rounded-md transition-colors"
            >
              Restart Now
            </button>
          </div>
        )}

        {status === 'error' && (
          <div className="space-y-2">
            <p className="text-sm text-red-600 dark:text-red-400">
              {error || 'Failed to check for updates'}
            </p>
            <button
              onClick={checkForUpdates}
              className="text-sm text-blue-600 hover:text-blue-700 dark:text-blue-400"
            >
              Try again
            </button>
          </div>
        )}

        <p className="text-xs text-gray-400 dark:text-gray-500">
          Current version: v{appVersion}
        </p>
      </div>
    </section>
  );
}

/**
 * Settings window component
 */
export function Settings() {
  const {
    aiPolishEnabled,
    telemetryEnabled,
    shortcut,
    handsFreeMode,
    hidePillWhenInactive,
    dictionary,
    history,
    loading,
    loadSettings,
    saveSettings,
    resetSettings,
    loadDictionary,
    deleteEntry,
    clearDictionary,
    loadHistory,
    clearHistory,
  } = useSettingsStore();

  const isMac = navigator.platform.startsWith('Mac');

  const [appVersion, setAppVersion] = useState('...');
  const updateSectionRef = useRef<HTMLDivElement>(null);

  // Load dynamic app version
  useEffect(() => {
    getVersion().then(v => setAppVersion(v)).catch(() => {});
  }, []);

  // Scroll to update section when update-available event fires
  useEffect(() => {
    const unlisten = listen<{ version: string; body?: string }>('update-available', () => {
      updateSectionRef.current?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  const [showClearConfirm, setShowClearConfirm] = useState(false);
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [showClearHistoryConfirm, setShowClearHistoryConfirm] = useState(false);
  const [shortcutError, setShortcutError] = useState('');
  const [shortcutSuccess, setShortcutSuccess] = useState(false);
  const [groqApiKey, setGroqApiKey] = useState('');
  const [hasGroqKey, setHasGroqKey] = useState(false);
  const [groqKeySaving, setGroqKeySaving] = useState(false);
  const [groqKeySuccess, setGroqKeySuccess] = useState(false);
  const [newOriginal, setNewOriginal] = useState('');
  const [newCorrection, setNewCorrection] = useState('');
  const [addEntryError, setAddEntryError] = useState('');
  const [showRestartBanner, setShowRestartBanner] = useState(false);

  // Check API key status
  const checkApiKeys = useCallback(() => {
    invoke<boolean>('has_groq_api_key').then(setHasGroqKey).catch(console.error);
  }, []);

  // Load settings, dictionary, and history on mount
  useEffect(() => {
    loadSettings();
    loadDictionary();
    loadHistory();
    checkApiKeys();
  }, [loadSettings, loadDictionary, loadHistory, checkApiKeys]);

  // Re-check API keys when window gets focus (e.g. after setup popup)
  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) checkApiKeys();
    });
    return () => { unlisten.then(fn => fn()); };
  }, [checkApiKeys]);

  // Refresh dictionary when backend auto-detects corrections
  useEffect(() => {
    const unlisten = listen('dictionary-changed', () => {
      loadDictionary();
    });
    return () => { unlisten.then(fn => fn()); };
  }, [loadDictionary]);

  // Refresh history when a transcription completes (state goes back to Idle)
  useEffect(() => {
    const unlisten = listen('recording-state-changed', (event) => {
      if (event.payload === 'Idle') {
        // Small delay to let the backend finish writing history
        setTimeout(() => loadHistory(), 500);
      }
    });
    return () => { unlisten.then(fn => fn()); };
  }, [loadHistory]);

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

  // Handle Groq API key save
  const handleGroqKeySave = async () => {
    if (!groqApiKey.trim()) return;
    setGroqKeySaving(true);
    try {
      await invoke('set_groq_api_key', { key: groqApiKey });
      setHasGroqKey(true);
      setGroqApiKey('');
      setGroqKeySuccess(true);
      setTimeout(() => setGroqKeySuccess(false), 3000);
    } catch (error) {
      console.error('Failed to save Groq API key:', error);
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
          setShortcutError('Input Monitoring permission required for Fn key. Go to System Settings → Privacy & Security → Input Monitoring and enable TTP, then restart the app.');
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
      setAddEntryError('Both fields are required');
      return;
    }
    if (orig === corr) {
      setAddEntryError('Original and correction must be different');
      return;
    }
    try {
      await invoke('add_dictionary_entry', { original: orig, correction: corr });
      setNewOriginal('');
      setNewCorrection('');
      await loadDictionary();
    } catch (error) {
      setAddEntryError(String(error));
    }
  };

  // Handle delete single entry
  const handleDeleteEntry = async (original: string) => {
    try {
      await deleteEntry(original);
    } catch (error) {
      console.error('Failed to delete entry:', error);
    }
  };

  // Handle clear history
  const handleClearHistory = async () => {
    try {
      await clearHistory();
      setShowClearHistoryConfirm(false);
    } catch (error) {
      console.error('Failed to clear history:', error);
    }
  };

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-gray-900 p-6">
      <div className="max-w-lg mx-auto">
        {/* Welcome / About */}
        <section className="bg-gradient-to-br from-gray-900 to-gray-800 rounded-lg shadow-sm p-6 mb-6 text-white">
          <h1 className="text-xl font-bold mb-1">TTP by AmirKS</h1>
          <p className="text-blue-400 text-xs font-medium mb-3">Talk To Paste — v{appVersion}</p>
          <p className="text-sm text-gray-300 leading-relaxed mb-3">
            Thanks for using TTP! This is a free, open-source app I built to make voice-to-text
            effortless. Just press your hotkey, speak, and your words are transcribed and pasted
            instantly — powered by Groq's fast AI, with smart polish to clean up filler words
            and grammar. TTP also learns from your corrections over time.
          </p>
          <p className="text-sm text-gray-300 leading-relaxed mb-3">
            Built by Amir KELLOU--SIDHOUM. If you find it useful, feel free to
            share it or connect with me!
          </p>
          <div className="p-3 bg-gray-700/50 rounded-lg mb-4">
            <p className="text-xs text-gray-400 leading-relaxed">
              <span className="text-green-400 font-medium">Privacy:</span> TTP collects absolutely
              none of your data. Your API key is stored locally on your machine only. Audio is sent
              directly from your device to Groq's servers for transcription — I never see, store,
              or have access to any of it. Same for AI Polish: your text goes straight to Groq,
              not through me. Everything stays between you and your API provider.
            </p>
          </div>
          <div className="flex gap-3">
            <a
              href="https://amirks.eu"
              target="_blank"
              rel="noopener noreferrer"
              className="px-3 py-1.5 text-xs font-medium bg-blue-600 hover:bg-blue-700 rounded-md transition-colors"
            >
              amirks.eu
            </a>
            <a
              href="https://www.linkedin.com/in/amirks/"
              target="_blank"
              rel="noopener noreferrer"
              className="px-3 py-1.5 text-xs font-medium bg-gray-700 hover:bg-gray-600 rounded-md transition-colors"
            >
              LinkedIn
            </a>
          </div>
        </section>

        {/* Recording Trigger Section */}
        <section className="bg-white dark:bg-gray-800 rounded-lg shadow-sm p-6 mb-6">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
            Recording Trigger
          </h2>
          <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
            Hold to record, release to stop. Double-tap to toggle hands-free mode.
          </p>

          <div className="space-y-2">
            {(isMac
              ? [
                  { value: 'FnKey', label: 'Fn', desc: 'Recommended', recommended: true },
                  { value: 'Alt+Space', label: '⌥ Space', desc: 'Option + Space' },
                  { value: 'CmdOrCtrl+Shift+R', label: '⌘⇧ R', desc: 'Cmd + Shift + R' },
                ]
              : [
                  { value: 'Super+Space', label: 'Win + Space', desc: 'Recommended (disable Windows language switcher)', recommended: true },
                  { value: 'Super+J', label: 'Win + J', desc: 'No conflicts' },
                  { value: 'Ctrl+Space', label: 'Ctrl + Space', desc: '' },
                  { value: 'Alt+Space', label: 'Alt + Space', desc: '' },
                ]
            ).map((opt) => (
              <button
                key={opt.value}
                onClick={() => handleShortcutChange(opt.value)}
                disabled={loading}
                className={`
                  w-full flex items-center justify-between px-4 py-3 rounded-lg border-2 transition-all text-left
                  ${shortcut === opt.value
                    ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20'
                    : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'
                  }
                `}
              >
                <div className="flex items-center gap-3">
                  <span className={`
                    w-4 h-4 rounded-full border-2 flex items-center justify-center flex-shrink-0
                    ${shortcut === opt.value
                      ? 'border-blue-500'
                      : 'border-gray-400 dark:border-gray-500'
                    }
                  `}>
                    {shortcut === opt.value && (
                      <span className="w-2 h-2 rounded-full bg-blue-500" />
                    )}
                  </span>
                  <span className={`font-mono text-sm font-semibold ${
                    shortcut === opt.value
                      ? 'text-blue-700 dark:text-blue-300'
                      : 'text-gray-900 dark:text-white'
                  }`}>
                    {opt.label}
                  </span>
                </div>
                {opt.desc && (
                  <span className={`text-xs ${
                    opt.recommended
                      ? 'text-blue-600 dark:text-blue-400 font-medium'
                      : 'text-gray-400 dark:text-gray-500'
                  }`}>
                    {opt.desc}
                  </span>
                )}
              </button>
            ))}
          </div>

          {shortcutError && (
            <p className="text-sm text-red-600 dark:text-red-400 mt-3">
              {shortcutError}
            </p>
          )}

          {shortcutSuccess && (
            <p className="text-sm text-green-600 dark:text-green-400 mt-3">
              Shortcut updated!
            </p>
          )}
        </section>

        {/* Recording Mode Section */}
        <section className="bg-white dark:bg-gray-800 rounded-lg shadow-sm p-6 mb-6">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
            Recording Mode
          </h2>

          {/* Hands-free mode toggle */}
          <div className="flex items-center justify-between mb-4">
            <div className="flex-1 pr-4">
              <p className="text-gray-900 dark:text-white font-medium">
                Hands-free mode (Toggle)
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                When enabled, press once to start recording, press again to stop
              </p>
            </div>
            <Toggle
              enabled={handsFreeMode}
              onChange={handleHandsFreeModeToggle}
              disabled={loading}
            />
          </div>

          {/* Hide pill when inactive toggle */}
          <div className="flex items-center justify-between">
            <div className="flex-1 pr-4">
              <p className="text-gray-900 dark:text-white font-medium">
                Hide pill when inactive
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                Hide the recording indicator when not recording
              </p>
            </div>
            <Toggle
              enabled={hidePillWhenInactive}
              onChange={handleHidePillWhenInactiveToggle}
              disabled={loading}
            />
          </div>
        </section>

        {/* Transcription Section */}
        <section className="bg-white dark:bg-gray-800 rounded-lg shadow-sm p-6 mb-6">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
            Transcription
          </h2>

          {/* Groq API Key */}
          <div className="mb-6 p-4 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
            <p className="text-gray-900 dark:text-white font-medium mb-2">
              Groq API Key
            </p>
              {hasGroqKey ? (
                <div className="flex items-center gap-2">
                  <span className="text-sm text-green-600 dark:text-green-400">✓ Key configured</span>
                  <button
                    onClick={() => setHasGroqKey(false)}
                    className="text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
                  >
                    Change
                  </button>
                </div>
              ) : (
                <div className="space-y-2">
                  <div className="flex gap-2">
                    <input
                      type="password"
                      value={groqApiKey}
                      onChange={(e) => setGroqApiKey(e.target.value)}
                      placeholder="gsk_..."
                      className="flex-1 px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500 text-sm"
                    />
                    <button
                      onClick={handleGroqKeySave}
                      disabled={groqKeySaving || !groqApiKey.trim()}
                      className="px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed rounded-md transition-colors"
                    >
                      Save
                    </button>
                  </div>
                  {groqKeySuccess && (
                    <p className="text-sm text-green-600 dark:text-green-400">
                      API key saved!
                    </p>
                  )}
                  <p className="text-xs text-gray-500 dark:text-gray-400">
                    Get your free key at{' '}
                    <a
                      href="https://console.groq.com/keys"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-blue-500 hover:underline"
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
              <p className="text-gray-900 dark:text-white font-medium">
                AI Polish
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                Clean up transcriptions with AI (remove filler words, fix grammar)
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
        <section className="bg-white dark:bg-gray-800 rounded-lg shadow-sm p-6 mb-6">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
            Privacy & Telemetry
          </h2>

          <div className="flex items-center justify-between mb-4">
            <div className="flex-1 pr-4">
              <p className="text-gray-900 dark:text-white font-medium">
                Help Improve TTP
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                Send anonymous crash reports and usage statistics
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
            <div className="flex items-center justify-between p-3 bg-amber-50 dark:bg-amber-900/20 rounded-lg mb-4">
              <p className="text-sm text-amber-700 dark:text-amber-400">
                Changes take effect after restart
              </p>
              <button
                onClick={() => relaunch().catch(console.error)}
                className="px-3 py-1.5 text-xs font-medium text-amber-700 dark:text-amber-400 border border-amber-300 dark:border-amber-700 hover:bg-amber-100 dark:hover:bg-amber-900/40 rounded-md transition-colors ml-3 whitespace-nowrap"
              >
                Restart Now
              </button>
            </div>
          )}

          {/* Privacy explanation */}
          <div className="p-3 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
            <p className="text-xs text-gray-500 dark:text-gray-400 leading-relaxed">
              <span className="font-medium text-gray-700 dark:text-gray-300">What is sent:</span>{' '}
              Crash reports (error type, OS, app version, pipeline stage)
              and anonymous usage events (feature usage counts).
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400 leading-relaxed mt-2">
              <span className="font-medium text-gray-700 dark:text-gray-300">Never sent:</span>{' '}
              Your transcription text, API keys, file paths, or any personal data.
            </p>
          </div>
        </section>

        {/* Dictionary Section */}
        <section className="bg-white dark:bg-gray-800 rounded-lg shadow-sm p-6 mb-6">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
              Dictionary
            </h2>
            {dictionary.length > 0 && (
              <button
                onClick={() => setShowClearConfirm(true)}
                className="text-sm text-red-600 hover:text-red-700 font-medium"
              >
                Clear All
              </button>
            )}
          </div>

          {/* Add entry form */}
          <div className="mb-4 flex gap-2 items-end">
            <div className="flex-1">
              <label className="block text-xs text-gray-500 dark:text-gray-400 mb-1">Misheard</label>
              <input
                type="text"
                value={newOriginal}
                onChange={(e) => setNewOriginal(e.target.value)}
                placeholder="grok"
                className="w-full px-3 py-1.5 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            </div>
            <span className="text-gray-400 pb-1.5">&rarr;</span>
            <div className="flex-1">
              <label className="block text-xs text-gray-500 dark:text-gray-400 mb-1">Correction</label>
              <input
                type="text"
                value={newCorrection}
                onChange={(e) => setNewCorrection(e.target.value)}
                placeholder="Groq"
                className="w-full px-3 py-1.5 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            </div>
            <button
              onClick={handleAddEntry}
              disabled={!newOriginal.trim() || !newCorrection.trim()}
              className="px-3 py-1.5 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed rounded-md transition-colors"
            >
              Add
            </button>
          </div>
          {addEntryError && (
            <p className="text-red-500 text-sm mb-3">{addEntryError}</p>
          )}

          {dictionary.length === 0 ? (
            <p className="text-gray-500 dark:text-gray-400 text-center py-4">
              No corrections yet
            </p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b border-gray-200 dark:border-gray-700">
                    <th className="text-left py-2 px-4 text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                      Original
                    </th>
                    <th className="text-left py-2 px-4 text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                      Correction
                    </th>
                    <th className="w-20"></th>
                  </tr>
                </thead>
                <tbody>
                  {dictionary.map((entry) => (
                    <DictionaryRow
                      key={entry.original}
                      entry={entry}
                      onDelete={() => handleDeleteEntry(entry.original)}
                    />
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>

        {/* History Section */}
        <section className="bg-white dark:bg-gray-800 rounded-lg shadow-sm p-6 mb-6">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
              Transcription History
            </h2>
            {history.length > 0 && (
              <button
                onClick={() => setShowClearHistoryConfirm(true)}
                className="text-sm text-red-600 hover:text-red-700 font-medium"
              >
                Clear History
              </button>
            )}
          </div>

          {history.length === 0 ? (
            <p className="text-gray-500 dark:text-gray-400 text-center py-8">
              No transcriptions yet
            </p>
          ) : (
            <div className="max-h-80 overflow-y-auto rounded-md border border-gray-200 dark:border-gray-700">
              {history.map((entry, index) => (
                <HistoryRow key={`${entry.timestamp}-${index}`} entry={entry} />
              ))}
            </div>
          )}
        </section>

        {/* Updates Section */}
        <div ref={updateSectionRef}>
          <UpdateSection />
        </div>

        {/* Reset Section */}
        <section className="bg-white dark:bg-gray-800 rounded-lg shadow-sm p-6">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
            Reset
          </h2>
          <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
            Reset all settings to their default values and clear the dictionary.
          </p>
          <button
            onClick={() => setShowResetConfirm(true)}
            className="px-4 py-2 text-sm font-medium text-red-600 border border-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-md transition-colors"
          >
            Reset to Defaults
          </button>
        </section>

        {/* Confirmation Dialogs */}
        <ConfirmDialog
          open={showClearConfirm}
          title="Clear Dictionary"
          message="Are you sure you want to delete all learned corrections? This cannot be undone."
          confirmText="Clear All"
          onConfirm={handleClearDictionary}
          onCancel={() => setShowClearConfirm(false)}
        />

        <ConfirmDialog
          open={showResetConfirm}
          title="Reset to Defaults"
          message="Are you sure you want to reset all settings and clear the dictionary? This cannot be undone."
          confirmText="Reset"
          onConfirm={handleResetDefaults}
          onCancel={() => setShowResetConfirm(false)}
        />

        <ConfirmDialog
          open={showClearHistoryConfirm}
          title="Clear History"
          message="Are you sure you want to delete all transcription history? This cannot be undone."
          confirmText="Clear History"
          onConfirm={handleClearHistory}
          onCancel={() => setShowClearHistoryConfirm(false)}
        />
      </div>
    </div>
  );
}

export default Settings;
