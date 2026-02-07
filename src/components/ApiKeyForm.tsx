// TTP - Talk To Paste
// API key input form component for first-run setup

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface Props {
  onSuccess: () => void;
}

/**
 * Form component for entering Groq and OpenAI API keys.
 * Groq is required (transcription), OpenAI is optional (text polish).
 */
export function ApiKeyForm({ onSuccess }: Props) {
  const [groqKey, setGroqKey] = useState('');
  const [openaiKey, setOpenaiKey] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (!groqKey.trim()) {
      setError('Groq API key is required for transcription');
      return;
    }
    if (!groqKey.startsWith('gsk_')) {
      setError('Groq API key should start with "gsk_"');
      return;
    }

    setLoading(true);
    try {
      // Save Groq key (required)
      await invoke('set_groq_api_key', { key: groqKey });

      // Save OpenAI key if provided (optional, for text polish)
      if (openaiKey.trim()) {
        if (!openaiKey.startsWith('sk-')) {
          setError('OpenAI API key should start with "sk-"');
          setLoading(false);
          return;
        }
        await invoke('set_api_key', { key: openaiKey });
      }

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
        <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
          Free — get yours at{' '}
          <a
            href="https://console.groq.com/keys"
            className="text-blue-500 hover:text-blue-600 underline"
            target="_blank"
            rel="noopener noreferrer"
          >
            console.groq.com
          </a>
        </p>
      </div>

      {/* OpenAI - Optional */}
      <div>
        <label
          htmlFor="openai-key"
          className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
        >
          OpenAI API Key <span className="text-gray-400 text-xs font-normal">(optional)</span>
        </label>
        <input
          id="openai-key"
          type="password"
          value={openaiKey}
          onChange={(e) => setOpenaiKey(e.target.value)}
          placeholder="sk-..."
          className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors"
          autoComplete="off"
        />
        <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
          For text correction & formatting.{' '}
          <a
            href="https://platform.openai.com/api-keys"
            className="text-blue-500 hover:text-blue-600 underline"
            target="_blank"
            rel="noopener noreferrer"
          >
            platform.openai.com
          </a>
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
        {loading ? 'Saving...' : 'Get Started'}
      </button>
    </form>
  );
}
