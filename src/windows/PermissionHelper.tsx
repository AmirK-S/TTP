// TTP - Talk To Paste
// The panel that sits on the bottom edge of System Settings and says: drag
// this into the list above. Rust owns the window and its position
// (`src-tauri/src/permission_helper.rs`); this is what it draws.
//
// The whole card is the drag handle. One row, one icon, one instruction, so
// it reads at a glance over a busy Settings window.

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { startDrag } from '@crabnebula/tauri-plugin-drag';
import { useTranslation } from 'react-i18next';
import { ArrowUp, X } from 'lucide-react';
import appIcon from '../../src-tauri/icons/128x128.png?inline';
import { useTauriEvent } from '../hooks/useTauriEvent';
import { safeInvoke } from '../lib/safeInvoke';
import { cn } from '../lib/cn';

type PermissionKind = 'accessibility' | 'inputMonitoring';

export function PermissionHelper() {
  const { t } = useTranslation();
  const [kind, setKind] = useState<PermissionKind | null>(null);
  const [bundlePath, setBundlePath] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);

  useEffect(() => {
    document.documentElement.classList.add('transparent-window');
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
    <div className="h-screen w-screen p-2.5 bg-transparent">
      <div
        onMouseDown={beginDrag}
        className={cn(
          'group relative flex h-full items-center gap-3.5 rounded-[16px] pl-3 pr-10 select-none cursor-grab active:cursor-grabbing',
          'bg-[#232326]/95 backdrop-blur-xl text-white ring-1 ring-white/[0.12]',
          'shadow-[0_8px_24px_rgba(0,0,0,0.35)] transition-opacity',
          dragging && 'opacity-60',
        )}
      >
        <img src={appIcon} alt="" draggable={false} className="size-11 shrink-0 rounded-[10px] shadow-sm" />
        <div className="min-w-0">
          <p className="flex items-center gap-1.5 text-[13px] font-semibold leading-tight whitespace-nowrap">
            <ArrowUp className="size-3.5 shrink-0 text-[#0a84ff]" strokeWidth={2.5} aria-hidden />
            {t('permissionHelper.instruction')}
          </p>
          <p className="mt-1 text-[12px] leading-tight text-white/55 truncate">
            {t('permissionHelper.dragHint', { permission })}
          </p>
        </div>
        <button
          type="button"
          onMouseDown={(e) => e.stopPropagation()}
          onClick={() => invoke('close_permission_helper').catch(() => {})}
          aria-label={t('common.cancel')}
          className="absolute right-2.5 top-1/2 -translate-y-1/2 rounded-full p-1.5 text-white/40 hover:text-white hover:bg-white/10 transition-colors"
        >
          <X className="size-3.5" aria-hidden />
        </button>
      </div>
    </div>
  );
}

export default PermissionHelper;
