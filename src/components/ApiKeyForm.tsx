// TTP - Talk To Paste
// API key input form component for first-run setup

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface Props {
  onSuccess: () => void;
}

/**
 * Form component for entering Groq API key.
 * Groq is the only required key (used for transcription + text polish).
 */
export function ApiKeyForm({ onSuccess }: Props) {
  const [groqKey, setGroqKey] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (!groqKey.trim()) {
      setError('Groq API key is required');
      return;
    }
    if (!groqKey.startsWith('gsk_')) {
      setError('Groq API key should start with "gsk_"');
      return;
    }

    setLoading(true);
    try {
      await invoke('validate_groq_api_key', { key: groqKey });
      await invoke('set_groq_api_key', { key: groqKey });
      onSuccess();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-5">
      {/* Groq - Required */}
      <div>
        <label
          htmlFor="groq-key"
          className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
        >
          Groq API Key <span className="text-red-500">*</span>
        </label>
        <input
          id="groq-key"
          type="password"
          value={groqKey}
          onChange={(e) => setGroqKey(e.target.value)}
          placeholder="gsk_..."
          className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors"
          autoComplete="off"
          autoFocus
        />
        <ol className="mt-2 text-xs text-gray-500 dark:text-gray-400 list-decimal pl-5 space-y-1">
          <li>
            Sign up at{' '}
            <a
              href="https://console.groq.com"
              className="text-blue-500 hover:text-blue-600 underline"
              target="_blank"
              rel="noopener noreferrer"
            >
              console.groq.com
            </a>{' '}
            (free, no credit card)
          </li>
          <li>Click "API Keys" → "Create API Key"</li>
          <li>
            Copy the key starting with{' '}
            <code className="px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-800 font-mono text-[11px]">
              gsk_
            </code>{' '}
            and paste it above
          </li>
        </ol>
        <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
          Groq's free tier covers tens of thousands of transcriptions per month — you'll never see
          a bill from typical usage.
        </p>
      </div>

      {error && (
        <p className="text-red-500 dark:text-red-400 text-sm">{error}</p>
      )}

      <button
        type="submit"
        disabled={loading || !groqKey}
        className="w-full py-2 px-4 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 text-white font-medium rounded-lg transition-colors disabled:cursor-not-allowed"
      >
        {loading ? 'Validating...' : 'Get Started'}
      </button>
    </form>
  );
}
