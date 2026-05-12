// TTP - Talk To Paste
// "What's New" modal — shown once after an app update

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';

interface WhatsNewData {
  version: string;
  changelog: string;
}

export default function WhatsNew() {
  const { t } = useTranslation();
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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
      <div className="bg-white dark:bg-gray-800 rounded-xl shadow-2xl max-w-md w-full max-h-[85vh] flex flex-col">
        <div className="px-6 pt-6 pb-4 shrink-0">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">
            {t('whatsNew.title', { version: data.version })}
          </h2>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            {t('whatsNew.subtitle')}
          </p>
        </div>

        <ul className="space-y-1.5 px-6 text-sm text-gray-700 dark:text-gray-300 overflow-y-auto flex-1 min-h-0">
          {lines.map((line, i) => (
            <li key={`${i}-${line.slice(0, 30)}`}>{line}</li>
          ))}
        </ul>

        <div className="px-6 pt-4 pb-6 shrink-0">
          <button
            onClick={dismiss}
            className="w-full rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2"
          >
            {t('whatsNew.dismiss')}
          </button>
        </div>
      </div>
    </div>
  );
}
