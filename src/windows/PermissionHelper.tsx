// TTP - Talk To Paste
// The panel that sits on the bottom edge of System Settings and says: drag
// this into the list above. Rust owns the window and its position
// (`src-tauri/src/permission_helper.rs`); this is what it draws.

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { startDrag } from '@crabnebula/tauri-plugin-drag';
import { useTranslation } from 'react-i18next';
import { ArrowUp, X } from 'lucide-react';
import appIcon from '../../src-tauri/icons/128x128.png?inline';
import { useTauriEvent } from '../hooks/useTauriEvent';
import { safeInvoke } from '../lib/safeInvoke';

type PermissionKind = 'accessibility' | 'inputMonitoring';

export function PermissionHelper() {
  const { t } = useTranslation();
  const [kind, setKind] = useState<PermissionKind | null>(null);
  const [bundlePath, setBundlePath] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);

  useEffect(() => {
    document.documentElement.style.background = 'transparent';
    document.body.style.background = 'transparent';
    safeInvoke<PermissionKind | null>('permission_helper_kind').then(setKind).catch(() => {});
    safeInvoke<string | null>('app_bundle_path').then(setBundlePath).catch(() => {});
  }, []);

  // Reused for the second permission without being recreated.
  useTauriEvent<PermissionKind>('permission-helper-kind', (event) => setKind(event.payload));

  const beginDrag = () => {
    if (!bundlePath) return;
    setDragging(true);
    startDrag({ item: [bundlePath], icon: appIcon })
      .catch((e) => console.error('[PermissionHelper] drag failed:', e))
      .finally(() => setDragging(false));
  };

  const permission = kind ? t(`onboarding.item.${kind}`) : '';

  return (
    <div className="h-screen w-screen p-2 bg-transparent">
      <div className="h-full rounded-[18px] bg-[#1f1f22]/92 backdrop-blur-xl px-4 py-3 text-white shadow-[0_12px_40px_rgba(0,0,0,0.45)] ring-1 ring-white/10 flex flex-col gap-2.5">
        <div className="flex items-start gap-2">
          <ArrowUp className="size-4 shrink-0 text-[#0a84ff] mt-0.5" aria-hidden />
          <p className="text-[13px] leading-snug">
            {t('permissionHelper.instruction', { permission })}
          </p>
          <button
            type="button"
            onClick={() => invoke('close_permission_helper').catch(() => {})}
            aria-label={t('common.cancel')}
            className="ml-auto shrink-0 rounded-full p-1 text-white/50 hover:text-white hover:bg-white/10 transition-colors"
          >
            <X className="size-3.5" aria-hidden />
          </button>
        </div>

        <div
          onMouseDown={beginDrag}
          role="img"
          aria-label={t('permissionHelper.dragHint')}
          className={
            'flex items-center gap-3 rounded-xl bg-white/10 px-3 py-2 select-none cursor-grab ' +
            'transition-colors hover:bg-white/[0.16] ' +
            (dragging ? 'opacity-60' : '')
          }
        >
          <img src={appIcon} alt="" draggable={false} className="size-9 rounded-app-sm" />
          <span className="text-[13px] font-medium">TTP by AmirKS</span>
          <span className="ml-auto text-[12px] text-white/50">{t('permissionHelper.dragHint')}</span>
        </div>
      </div>
    </div>
  );
}

export default PermissionHelper;
