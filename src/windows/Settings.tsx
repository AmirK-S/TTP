// TTP - Talk To Paste
// Settings window - configure app behavior and manage dictionary

import { useEffect, useState } from 'react';
import { Copy, Check } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useSettingsStore, DictionaryEntry, HistoryEntry, TranscriptionProvider } from '../stores/settings-store';

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
 * Settings window component
 */
export function Settings() {
  const {
    aiPolishEnabled,
    shortcut,
    transcriptionProvider,
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

  const [showClearConfirm, setShowClearConfirm] = useState(false);
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [showClearHistoryConfirm, setShowClearHistoryConfirm] = useState(false);
  const [shortcutInput, setShortcutInput] = useState('');
  const [shortcutError, setShortcutError] = useState('');
  const [shortcutSuccess, setShortcutSuccess] = useState(false);
  const [groqApiKey, setGroqApiKey] = useState('');
  const [hasGroqKey, setHasGroqKey] = useState(false);
  const [groqKeySaving, setGroqKeySaving] = useState(false);
  const [groqKeySuccess, setGroqKeySuccess] = useState(false);

  // Load settings, dictionary, and history on mount
  useEffect(() => {
    loadSettings();
    loadDictionary();
    loadHistory();
    // Check if Groq API key exists
    invoke<boolean>('has_groq_api_key').then(setHasGroqKey).catch(console.error);
  }, [loadSettings, loadDictionary, loadHistory]);

  // Sync shortcut input with store value
  useEffect(() => {
    setShortcutInput(shortcut);
  }, [shortcut]);

  // Handle AI polish toggle
  const handlePolishToggle = async (enabled: boolean) => {
    try {
      await saveSettings({ ai_polish_enabled: enabled });
    } catch (error) {
      console.error('Failed to save AI polish setting:', error);
    }
  };

  // Handle provider change
  const handleProviderChange = async (provider: TranscriptionProvider) => {
    try {
      await saveSettings({ transcription_provider: provider });
    } catch (error) {
      console.error('Failed to save provider setting:', error);
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

  // Handle shortcut update
  const handleShortcutApply = async () => {
    setShortcutError('');
    setShortcutSuccess(false);

    try {
      // First try to update the active shortcut
      await invoke('update_shortcut_cmd', { shortcut: shortcutInput });

      // If successful, save to settings
      await saveSettings({ shortcut: shortcutInput });

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
        {/* Header */}
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white mb-6">
          Settings
        </h1>

        {/* Keyboard Shortcut Section */}
        <section className="bg-white dark:bg-gray-800 rounded-lg shadow-sm p-6 mb-6">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
            Keyboard Shortcut
          </h2>
          <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
            Press and hold to record (push-to-talk), or double-tap to toggle recording
          </p>

          <div className="space-y-3">
            <div className="flex items-center gap-3">
              <input
                type="text"
                value={shortcutInput}
                onChange={(e) => setShortcutInput(e.target.value)}
                placeholder="Alt+Space"
                className="flex-1 px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500 font-mono text-sm"
              />
              <button
                onClick={handleShortcutApply}
                disabled={loading || shortcutInput === shortcut}
                className="px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed rounded-md transition-colors"
              >
                Apply
              </button>
            </div>

            {shortcutError && (
              <p className="text-sm text-red-600 dark:text-red-400">
                {shortcutError}
              </p>
            )}

            {shortcutSuccess && (
              <p className="text-sm text-green-600 dark:text-green-400">
                Shortcut updated successfully!
              </p>
            )}

            <p className="text-xs text-gray-400 dark:text-gray-500">
              Format: Alt+Space, Ctrl+Shift+R, CmdOrCtrl+Space
            </p>
          </div>
        </section>

        {/* Transcription Section */}
        <section className="bg-white dark:bg-gray-800 rounded-lg shadow-sm p-6 mb-6">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
            Transcription
          </h2>

          {/* Provider Selection */}
          <div className="mb-6">
            <p className="text-gray-900 dark:text-white font-medium mb-2">
              Provider
            </p>
            <div className="flex gap-3">
              <button
                onClick={() => handleProviderChange('groq')}
                className={`flex-1 px-4 py-3 rounded-lg border-2 transition-colors ${
                  transcriptionProvider === 'groq'
                    ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20'
                    : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'
                }`}
              >
                <p className="font-medium text-gray-900 dark:text-white">Groq</p>
                <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                  Fast (~0.3s) • Free tier
                </p>
              </button>
              <button
                onClick={() => handleProviderChange('openai')}
                className={`flex-1 px-4 py-3 rounded-lg border-2 transition-colors ${
                  transcriptionProvider === 'openai'
                    ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20'
                    : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'
                }`}
              >
                <p className="font-medium text-gray-900 dark:text-white">OpenAI</p>
                <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                  Accurate (~2-5s) • Paid
                </p>
              </button>
            </div>
          </div>

          {/* Groq API Key */}
          {transcriptionProvider === 'groq' && (
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
          )}

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

          {dictionary.length === 0 ? (
            <p className="text-gray-500 dark:text-gray-400 text-center py-8">
              No learned corrections yet
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
