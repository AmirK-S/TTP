// TTP - Talk To Paste
// First-launch onboarding wizard.
//
// Three steps: Welcome → Permissions → API key. Gating is intentionally soft:
// only Microphone and the API key are required to finish. Accessibility +
// Input Monitoring are strong recommendations but skippable — the user can
// grant them later from the in-app permission banner. Forcing them at
// install-time bricks the wizard for users who don't realise they have to
// re-click into System Settings to flip a toggle.

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useTranslation } from 'react-i18next';
import {
  Mic,
  Accessibility as AccessibilityIcon,
  Keyboard,
  CheckCircle2,
  ChevronRight,
  ChevronLeft,
  ExternalLink,
} from 'lucide-react';
import { Button, Card } from '../components/ui';
import { ApiKeyForm } from '../components/ApiKeyForm';
import { cn } from '../lib/cn';

type PermissionStatus = 'Granted' | 'Denied' | 'Undetermined';
type PermKey = 'microphone' | 'accessibility' | 'inputMonitoring';

const IS_MAC = typeof navigator !== 'undefined' && navigator.platform.startsWith('Mac');

const SETTINGS_COMMAND: Record<PermKey, string> = {
  microphone: 'open_microphone_settings',
  accessibility: 'open_accessibility_settings',
  inputMonitoring: 'open_input_monitoring_settings',
};

const PERM_ICON: Record<PermKey, typeof Mic> = {
  microphone: Mic,
  accessibility: AccessibilityIcon,
  inputMonitoring: Keyboard,
};

export default function Onboarding() {
  const { t } = useTranslation();
  // Allow the dev-only `?preview=onboarding&step=N` URL to jump straight to
  // a step for visual inspection (used by the screenshot tooling). Default
  // remains step 0 — production code never carries this query string.
  const initialStep = (() => {
    if (typeof window === 'undefined') return 0;
    const p = new URLSearchParams(window.location.search).get('step');
    const n = p ? Number(p) : NaN;
    return n === 1 || n === 2 ? n : 0;
  })();
  const [step, setStep] = useState<0 | 1 | 2>(initialStep as 0 | 1 | 2);
  const [hasApiKey, setHasApiKey] = useState(false);
  const [permStatus, setPermStatus] = useState<Record<PermKey, PermissionStatus>>({
    microphone: 'Undetermined',
    accessibility: 'Denied',
    inputMonitoring: 'Denied',
  });
  const [checking, setChecking] = useState<PermKey | null>(null);

  // Keep window title in sync with the active language. Wrapped because
  // getCurrentWindow() throws when the bundle is opened outside Tauri (the
  // ?preview= dev path used for screenshots), and the wizard should still
  // render for visual inspection.
  useEffect(() => {
    try { getCurrentWindow().setTitle(t('windowTitle.onboarding')); }
    catch { /* not in Tauri (dev preview) */ }
  });

  const refreshAll = useCallback(async () => {
    try {
      const inputMonProm = IS_MAC
        ? invoke<boolean>('check_input_monitoring_permission')
        : Promise.resolve(true);
      const [mic, key, ax, im] = await Promise.all([
        invoke<PermissionStatus>('check_microphone_permission'),
        invoke<boolean>('has_groq_api_key'),
        invoke<PermissionStatus>('check_accessibility_permission'),
        inputMonProm,
      ]);
      setHasApiKey(key);
      setPermStatus({
        microphone: mic,
        accessibility: ax,
        inputMonitoring: im ? 'Granted' : 'Denied',
      });
    } catch (e) {
      console.error('Onboarding refresh failed:', e);
    }
  }, []);

  useEffect(() => { refreshAll(); }, [refreshAll]);

  // Re-check whenever the window regains focus — catches returns from System
  // Settings without a polling timer.
  useEffect(() => {
    try {
      const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
        if (focused) refreshAll();
      });
      return () => { unlisten.then(fn => fn()); };
    } catch {
      // Non-Tauri runtime (dev preview): focus events are skipped.
      return undefined;
    }
  }, [refreshAll]);

  const openSettings = async (key: PermKey) => {
    try { await invoke(SETTINGS_COMMAND[key]); }
    catch (e) { console.error('Failed to open settings pane:', e); }
  };

  const requestMicrophone = async () => {
    setChecking('microphone');
    try {
      if (permStatus.microphone === 'Denied') await openSettings('microphone');
      else await invoke('request_microphone_permission');
    } catch (e) {
      console.log('Microphone permission result:', e);
    } finally {
      setChecking(null);
      // Race the OS prompt — refresh after a beat so the dot flips.
      setTimeout(refreshAll, 250);
    }
  };

  const requestAccessibility = async () => {
    setChecking('accessibility');
    try {
      if (permStatus.accessibility === 'Denied') await openSettings('accessibility');
      else await invoke('request_accessibility_permission');
    } catch (e) {
      console.log('Accessibility permission result:', e);
    } finally {
      setChecking(null);
      setTimeout(refreshAll, 250);
    }
  };

  const requestInputMonitoring = async () => {
    setChecking('inputMonitoring');
    try {
      const granted = await invoke<boolean>('request_input_monitoring_permission');
      if (!granted) await openSettings('inputMonitoring');
    } catch (e) {
      console.log('Input Monitoring permission result:', e);
    } finally {
      setChecking(null);
      setTimeout(refreshAll, 250);
    }
  };

  const finish = async () => {
    try { await invoke('close_onboarding'); }
    catch (e) { console.error('Failed to close onboarding:', e); }
  };

  const micGranted = permStatus.microphone === 'Granted';

  return (
    <div className="min-h-screen flex flex-col bg-app-bg">
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-xl mx-auto px-8 pt-16 pb-8">
          {step === 0 && <WelcomeStep />}
          {step === 1 && (
            <PermissionsStep
              permStatus={permStatus}
              checking={checking}
              onRequest={(key) => {
                if (key === 'microphone') return requestMicrophone();
                if (key === 'accessibility') return requestAccessibility();
                return requestInputMonitoring();
              }}
            />
          )}
          {step === 2 && (
            <ApiKeyStep
              hasApiKey={hasApiKey}
              onSaved={() => { setHasApiKey(true); finish(); }}
            />
          )}
        </div>
      </div>

      {/* Footer is intentionally DIMMER than the canvas — chrome recedes,
          content stays the brightest area (Linear 2026 refresh pattern). */}
      <footer className="shrink-0 border-t border-app-border bg-app-dim">
        <div className="max-w-xl mx-auto px-8 py-4 flex items-center justify-between gap-4">
          <StepDots current={step} total={3} />
          <div className="flex items-center gap-2">
            {step > 0 && (
              <Button
                variant="ghost"
                size="md"
                leftIcon={<ChevronLeft className="size-4" />}
                onClick={() => setStep((s) => (s - 1) as 0 | 1 | 2)}
              >
                {t('onboarding.wizard.back')}
              </Button>
            )}
            {step === 0 && (
              <Button
                size="md"
                rightIcon={<ChevronRight className="size-4" />}
                onClick={() => setStep(1)}
              >
                {t('onboarding.wizard.next')}
              </Button>
            )}
            {step === 1 && (
              <Button
                size="md"
                rightIcon={<ChevronRight className="size-4" />}
                onClick={() => setStep(2)}
                disabled={!micGranted}
                title={!micGranted ? t('onboarding.cta.stillMissing', { items: t('onboarding.item.microphone') }) : undefined}
              >
                {t('onboarding.wizard.next')}
              </Button>
            )}
            {step === 2 && hasApiKey && (
              <Button size="md" onClick={finish}>
                {t('onboarding.wizard.finish')}
              </Button>
            )}
          </div>
        </div>
      </footer>
    </div>
  );
}

/* ----------------------------- Step 0: Welcome ---------------------------- */

function WelcomeStep() {
  const { t } = useTranslation();
  return (
    <section className="anim-fade-up text-center max-w-md mx-auto">
      {/* Hero tile — surface ladder + inset highlight + accent glyph. The
          earlier flat white block was the brightest pixel on screen,
          punching above the H1. Now the tile sits IN the page surface
          with the wordmark in accent. */}
      <div
        className={cn(
          'mx-auto size-16 rounded-app-xl mb-7 shine-sm border border-app-border',
          'bg-app-surface grid place-items-center',
        )}
      >
        <span className="text-app-accent font-semibold text-[18px] tracking-[-0.022em]">TTP</span>
      </div>
      <h1 className="text-display-md text-app-text">
        {t('onboarding.wizard.welcomeTitle')}
      </h1>
      <p className="mt-2 text-[13px] text-app-muted leading-relaxed">
        {t('onboarding.wizard.welcomeSubtitle')}
      </p>

      <ul className="mt-8 text-left space-y-3.5">
        {[
          t('onboarding.wizard.welcomeBullet1'),
          t('onboarding.wizard.welcomeBullet2'),
          t('onboarding.wizard.welcomeBullet3'),
        ].map((bullet, i) => (
          <li
            key={i}
            className="flex items-start gap-3 anim-fade-up"
            style={{ animationDelay: `${0.08 + i * 0.06}s` }}
          >
            <CheckCircle2 className="size-4 mt-0.5 shrink-0 text-app-accent" aria-hidden />
            <span className="text-[13px] text-app-text leading-relaxed">{bullet}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}

/* --------------------------- Step 1: Permissions -------------------------- */

interface PermStepProps {
  permStatus: Record<PermKey, PermissionStatus>;
  checking: PermKey | null;
  onRequest: (key: PermKey) => void;
}

function PermissionsStep({ permStatus, checking, onRequest }: PermStepProps) {
  const { t } = useTranslation();
  const items: PermKey[] = IS_MAC
    ? ['microphone', 'accessibility', 'inputMonitoring']
    : ['microphone'];

  return (
    <section className="anim-fade-up">
      <h2 className="text-display-sm text-app-text">
        {t('onboarding.wizard.permissionsTitle')}
      </h2>
      <p className="mt-2 text-[13px] text-app-muted leading-relaxed">
        {t('onboarding.wizard.permissionsSubtitle')}
      </p>

      <div className="mt-7 space-y-2">
        {items.map((key, i) => (
          <PermissionRow
            key={key}
            permKey={key}
            status={permStatus[key]}
            isChecking={checking === key}
            onClick={() => onRequest(key)}
            delay={i}
          />
        ))}
      </div>
    </section>
  );
}

interface PermRowProps {
  permKey: PermKey;
  status: PermissionStatus;
  isChecking: boolean;
  onClick: () => void;
  delay: number;
}

/* Per-permission icon-tile tint. Borrowed from Raycast's settings layout
   where each row gets a distinctive accent so the column scans at a glance
   rather than reading as a uniform gray list. */
const PERM_TILE: Record<PermKey, string> = {
  microphone: 'bg-app-danger-tint text-app-danger',
  accessibility: 'bg-app-accent-tint text-app-accent',
  inputMonitoring: 'bg-app-success-tint text-app-success',
};

function PermissionRow({ permKey, status, isChecking, onClick, delay }: PermRowProps) {
  const { t } = useTranslation();
  const Icon = PERM_ICON[permKey];
  const granted = status === 'Granted';
  const denied = status === 'Denied';

  return (
    <Card
      elevation="sm"
      className="anim-fade-up flex items-center gap-4 px-5 py-4"
      style={{ animationDelay: `${0.06 + delay * 0.05}s` }}
    >
      <div className={cn(
        'shrink-0 size-9 rounded-app-md grid place-items-center transition-colors duration-200',
        granted ? 'bg-app-success-tint text-app-success' : PERM_TILE[permKey],
      )}>
        {granted
          ? <CheckCircle2 className="size-[18px]" aria-hidden />
          : <Icon className="size-[18px]" strokeWidth={1.75} aria-hidden />}
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-[13px] font-medium text-app-text tracking-[-0.005em]">
            {t(`onboarding.item.${permKey}`)}
          </span>
          {/* Status: tiny dot + short label. The previous "Denied — tap to
              open Settings" pill wrapped to two lines on long labels. The
              CTA-bearing button right of the row is now the only place
              with action copy; the status itself is just a state badge. */}
          <span className={cn(
            'inline-flex items-center gap-1.5 text-[11px] font-medium',
            granted && 'text-app-success',
            !granted && denied && 'text-app-danger',
            !granted && !denied && 'text-app-faint',
          )}>
            <span className={cn(
              'size-1.5 rounded-full',
              granted && 'bg-app-success',
              !granted && denied && 'bg-app-danger',
              !granted && !denied && 'bg-app-faint',
            )} aria-hidden />
            {granted
              ? t('onboarding.status.enabled')
              : denied
                ? t('onboarding.status.deniedShort')
                : t('onboarding.status.notEnabled')}
          </span>
        </div>
        <p className="mt-1 text-[12px] text-app-muted leading-snug">
          {t(`onboarding.help.${permKey}`)}
        </p>
      </div>
      {!granted && (
        <Button
          size="sm"
          variant="secondary"
          loading={isChecking}
          onClick={onClick}
          rightIcon={denied ? <ExternalLink className="size-3" /> : undefined}
        >
          {denied ? t('onboarding.button.openSettings') : t('onboarding.button.enable')}
        </Button>
      )}
    </Card>
  );
}

/* ---------------------------- Step 2: API key ----------------------------- */

interface ApiKeyStepProps {
  hasApiKey: boolean;
  onSaved: () => void;
}

function ApiKeyStep({ hasApiKey, onSaved }: ApiKeyStepProps) {
  const { t } = useTranslation();
  if (hasApiKey) {
    return (
      <section className="anim-fade-up text-center py-12">
        <div className="mx-auto size-14 rounded-full bg-app-success-soft grid place-items-center mb-4">
          <CheckCircle2 className="size-7 text-app-success" aria-hidden />
        </div>
        <h2 className="text-xl font-semibold tracking-tight text-app-text">
          {t('onboarding.apiKey.saved')}
        </h2>
        <p className="mt-2 text-sm text-app-muted">{t('onboarding.cta.reopenHint')}</p>
      </section>
    );
  }

  return (
    <section className="anim-fade-up">
      <h2 className="text-display-sm text-app-text">
        {t('onboarding.wizard.apiKeyTitle')}
      </h2>
      <p className="mt-2 text-[13px] text-app-muted leading-relaxed">
        {t('onboarding.wizard.apiKeySubtitle')}
      </p>
      <div className="mt-7">
        <ApiKeyForm onSuccess={onSaved} submitLabel={t('onboarding.wizard.finish')} />
      </div>
    </section>
  );
}

/* ---------------------------- Footer dots ------------------------------- */

interface StepDotsProps { current: number; total: number; }

/* 4×4 dot indicators with a 1px ring around the active step. The previous
   stretched-bar pattern (24×6 active, 6×6 inactive) was the shadcn-form
   2020 default — Raycast and Linear keep all dots the same size and signal
   active state via a faint accent ring instead. */
function StepDots({ current, total }: StepDotsProps) {
  const { t } = useTranslation();
  return (
    <div
      className="flex items-center gap-2"
      role="progressbar"
      aria-valuenow={current + 1}
      aria-valuemin={1}
      aria-valuemax={total}
      aria-label={t('onboarding.wizard.stepIndicatorLabel', { current: current + 1, total })}
    >
      {Array.from({ length: total }).map((_, i) => {
        const isActive = i === current;
        const isDone = i < current;
        return (
          <span
            key={i}
            className={cn(
              'relative size-1.5 rounded-full transition-colors duration-200 ease-[cubic-bezier(0.32,0.72,0,1)]',
              isActive && 'bg-app-accent',
              isDone && 'bg-app-accent/40',
              !isActive && !isDone && 'bg-app-border-strong',
            )}
          >
            {isActive && (
              <span
                aria-hidden
                className="absolute -inset-1 rounded-full ring-1 ring-app-accent/30"
              />
            )}
          </span>
        );
      })}
    </div>
  );
}
