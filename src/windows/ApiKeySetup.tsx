// TTP - Talk To Paste
// First-run API key setup window. Reached when the user has finished
// onboarding (or skipped it past v1.x) but cleared their Groq key — the
// fallback recovery path. Visually matches the onboarding wizard's API
// key step so the experience is one coherent flow.

import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { ApiKeyForm } from '../components/ApiKeyForm';
import { BrandTile } from '../components/ui';

export function ApiKeySetup() {
  const { t } = useTranslation();

  useEffect(() => {
    try { getCurrentWindow().setTitle(t('windowTitle.setup')); }
    catch { /* not in Tauri (dev preview) */ }
  }, [t]);

  const handleSuccess = async () => {
    try { await invoke('open_settings_window'); }
    catch (e) { console.error('Failed to open settings window:', e); }
    try {
      const window = getCurrentWindow();
      await window.close();
    } catch { /* not in Tauri */ }
  };

  return (
    <div className="min-h-screen flex flex-col bg-app-bg bg-noise">
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-xl mx-auto px-8 pt-16 pb-8">
          <section className="anim-fade-up max-w-md mx-auto">
            <BrandTile size="lg" className="mx-auto mb-7" />

            <h1 className="text-display-md text-app-text text-center">
              {t('setup.title')}
            </h1>
            <p className="mt-2 text-[13px] text-app-muted leading-relaxed text-center">
              {t('setup.description')}
            </p>

            <div className="mt-7">
              <ApiKeyForm onSuccess={handleSuccess} submitLabel={t('common.getStarted')} />
            </div>

            <p className="mt-6 text-[11px] text-app-faint leading-relaxed text-center">
              {t('setup.localStorage')}
            </p>
          </section>
        </div>
      </div>
    </div>
  );
}

export default ApiKeySetup;
