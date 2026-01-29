// TTP - Talk To Paste
// API key input form component for first-run setup

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface Props {
  onSuccess: () => void;
}

/**
 * Form component for entering and saving an OpenAI API key.
 * Validates format (must start with "sk-") and stores in system keychain.
 */
export function ApiKeyForm({ onSuccess }: Props) {
  const [apiKey, setApiKey] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    // Basic format validation
    if (!apiKey.trim()) {
      setError('Please enter your API key');
      return;
    }
    if (!apiKey.startsWith('sk-')) {
      setError('API key should start with "sk-"');
      return;
    }

    setLoading(true);
    try {
      await invoke('set_api_key', { key: apiKey });
      onSuccess();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <div>
        <label
          htmlFor="api-key"
          className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
        >
          OpenAI API Key
        </label>
        <input
          id="api-key"
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          placeholder="sk-..."
          className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors"
          autoComplete="off"
          autoFocus
        />
      </div>

      {error && (
        <p className="text-red-500 dark:text-red-400 text-sm">{error}</p>
      )}

      <button
        type="submit"
        disabled={loading || !apiKey}
        className="w-full py-2 px-4 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 text-white font-medium rounded-lg transition-colors disabled:cursor-not-allowed"
      >
        {loading ? 'Saving...' : 'Save API Key'}
      </button>

      <p className="text-xs text-gray-500 dark:text-gray-400">
        Get your API key from{' '}
        <a
          href="https://platform.openai.com/api-keys"
          className="text-blue-500 hover:text-blue-600 underline"
          target="_blank"
          rel="noopener noreferrer"
        >
          OpenAI Platform
        </a>
        . Your key is stored securely in your system keychain.
      </p>
    </form>
  );
}
