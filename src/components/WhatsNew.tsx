// TTP - Talk To Paste
// "What's New" modal — shown once after an app update

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface WhatsNewData {
  version: string;
  changelog: string;
}

export default function WhatsNew() {
  const [data, setData] = useState<WhatsNewData | null>(null);

  useEffect(() => {
    invoke<[string, string] | null>('check_whats_new').then((result) => {
      if (result) {
        setData({ version: result[0], changelog: result[1] });
      }
    }).catch((err) => {
      console.error('[WhatsNew] Failed to check:', err);
    });
  }, []);

  if (!data) return null;

  const dismiss = async () => {
    try {
      await invoke('dismiss_whats_new');
    } catch (err) {
      console.error('[WhatsNew] Failed to dismiss:', err);
    }
    setData(null);
  };

  const lines = data.changelog.split('\n');

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-white dark:bg-gray-800 rounded-xl shadow-2xl p-6 max-w-md mx-4 w-full">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">
          What&apos;s New in v{data.version}
        </h2>
        <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
          TTP has been updated!
        </p>

        <ul className="space-y-1.5 mb-6 text-sm text-gray-700 dark:text-gray-300">
          {lines.map((line, i) => (
            <li key={i}>{line}</li>
          ))}
        </ul>

        <button
          onClick={dismiss}
          className="w-full rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2"
        >
          Got it
        </button>
      </div>
    </div>
  );
}
