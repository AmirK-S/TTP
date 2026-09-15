// TTP - Talk To Paste
// First-launch onboarding wizard.
//
// Three steps: Permissions → API key → Try it. Gating is intentionally soft:
// only Microphone and the API key are required to advance. Accessibility +
// Input Monitoring are strong recommendations but skippable. The last step
// is a real dictation into a text box, so the user leaves having seen it work.

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
  Sparkles,
  ScanText,
  ShieldCheck,
} from 'lucide-react';
import { Button, Card, BrandTile, Toggle } from '../components/ui';
import { ApiKeyForm } from '../components/ApiKeyForm';
import { cn } from '../lib/cn';
import { useTauriEvent } from '../hooks/useTauriEvent';
import { TriggerPicker } from '../components/TriggerPicker';
import { useTrigger } from '../hooks/useTrigger';

type PermissionStatus = 'Granted' | 'Denied' | 'Undetermined';
type PermKey = 'microphone' | 'accessibility' | 'inputMonitoring';
type StepIndex = 0 | 1 | 2 | 3;

const TOTAL_STEPS = 4;
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
  // Dev-only `?preview=onboarding&step=N` jump for screenshot tooling.
  const initialStep = (() => {
    if (typeof window === 'undefined') return 0;
    const p = new URLSearchParams(window.location.search).get('step');
    const n = p ? Number(p) : NaN;
    return n >= 0 && n <= 3 ? (n as StepIndex) : 0;
  })();
  const [step, setStep] = useState<StepIndex>(initialStep);
  // `?trial=1` forces the post-save success view for screenshots.
  const initialHasKey = typeof window !== 'undefined' &&
    new URLSearchParams(window.location.search).get('trial') === '1';
  const [hasApiKey, setHasApiKey] = useState(initialHasKey);
  const [permStatus, setPermStatus] = useState<Record<PermKey, PermissionStatus>>({
    microphone: 'Undetermined',
    accessibility: 'Denied',
    inputMonitoring: 'Denied',
  });
  const [checking, setChecking] = useState<PermKey | null>(null);

  useEffect(() => {
    try { getCurrentWindow().setTitle(t('windowTitle.onboarding')); }
    catch { /* not in Tauri (dev preview) */ }
  }, [t]);

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
      setPermStatus({ microphone: mic, accessibility: ax, inputMonitoring: im ? 'Granted' : 'Denied' });
    } catch (e) {
      console.error('Onboarding refresh failed:', e);
    }
  }, []);

  useEffect(() => { refreshAll(); }, [refreshAll]);

  // The helper closes itself the moment the permission lands; this is what
  // ticks the row without waiting for the next poll.
  useTauriEvent('permission-granted', () => { refreshAll(); });

  useEffect(() => {
    try {
      const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
        if (focused) refreshAll();
      });
      return () => { unlisten.then(fn => fn()); };
    } catch {
      return undefined;
    }
  }, [refreshAll]);

  // Permissions step: poll once per second so the user can toggle perms in
  // System Settings and see the wizard update live (Wispr-style seamless flow).
  useEffect(() => {
    if (step !== 0) return;
    const id = window.setInterval(refreshAll, 1000);
    return () => window.clearInterval(id);
  }, [step, refreshAll]);

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
      setTimeout(refreshAll, 250);
    }
  };

  // Accessibility and Input Monitoring both take the drag panel: the system
  // prompt first (it puts TTP in the list, unchecked, which is often all that
  // is needed), then System Settings on the right page with the panel beside
  // it. `permission_helper` returns without a panel on a dev binary, where
  // there is no bundle to drag.
  const requestAccessibility = async () => {
    setChecking('accessibility');
    try {
      if (permStatus.accessibility !== 'Denied') await invoke('request_accessibility_permission');
      await invoke('show_permission_helper', { kind: 'accessibility' });
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
      if (!granted) await invoke('show_permission_helper', { kind: 'inputMonitoring' });
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
  const advance = () => setStep((s) => Math.min(3, s + 1) as StepIndex);
  const back = () => setStep((s) => Math.max(0, s - 1) as StepIndex);

  return (
    <div className="min-h-screen flex flex-col bg-app-bg bg-noise">
      <div className="flex-1 overflow-y-auto">
        {/* `key={step}` forces a remount so the anim-fade-up entry replays.
            A poor man's crossfade without framer-motion. */}
        <div key={step} className="max-w-xl mx-auto px-8 pt-16 pb-8">
          {step === 0 && (
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
          {step === 1 && (
            <ApiKeyStep hasApiKey={hasApiKey} onSaved={() => setHasApiKey(true)} />
          )}
          {step === 2 && <ScreenStep onChosen={advance} />}
          {step === 3 && <TryStep />}
        </div>
      </div>

      <footer className="shrink-0 border-t border-app-border bg-app-dim">
        <div className="max-w-xl mx-auto px-8 py-4 flex items-center justify-between gap-4">
          <StepDots current={step} total={TOTAL_STEPS} />
          <div className="flex items-center gap-2">
            {step > 0 && (
              <Button
                variant="ghost"
                size="md"
                leftIcon={<ChevronLeft className="size-4" />}
                onClick={back}
              >
                {t('onboarding.wizard.back')}
              </Button>
            )}
            {step === 0 && (
              <Button
                size="md"
                rightIcon={<ChevronRight className="size-4" />}
                onClick={advance}
                disabled={!micGranted}
                title={!micGranted ? t('onboarding.cta.stillMissing', { items: t('onboarding.item.microphone') }) : undefined}
              >
                {t('onboarding.wizard.next')}
              </Button>
            )}
            {step === 1 && hasApiKey && (
              <Button size="md" rightIcon={<ChevronRight className="size-4" />} onClick={advance}>
                {t('onboarding.wizard.next')}
              </Button>
            )}
            {step === 3 && (
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

/* --------------------------- Step 0: Permissions -------------------------- */

interface PermStepProps {
  permStatus: Record<PermKey, PermissionStatus>;
  checking: PermKey | null;
  onRequest: (key: PermKey) => void;
}

function PermissionsStep({ permStatus, checking, onRequest }: PermStepProps) {
  const { t } = useTranslation();
  const items: PermKey[] = IS_MAC ? ['microphone', 'accessibility', 'inputMonitoring'] : ['microphone'];

  return (
    <section className="anim-fade-up">
      <BrandTile size="md" className="mb-5" />
      <h1 className="text-display-sm text-app-text">{t('onboarding.wizard.welcomeTitle')}</h1>
      <p className="mt-2 text-[13px] text-app-muted leading-relaxed">{t('onboarding.wizard.permissionsSubtitle')}</p>

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
      className={cn(
        'anim-fade-up flex items-center gap-4 px-5 py-4',
        'transition-[background-color,transform] duration-hover ease-app-out',
        !granted && 'hover:bg-app-raised',
      )}
      style={{ animationDelay: `${0.06 + delay * 0.05}s` }}
    >
      <div className={cn(
        'shrink-0 size-9 rounded-app-md grid place-items-center transition-colors duration-hover',
        granted ? 'bg-app-success-tint text-app-success' : PERM_TILE[permKey],
      )}>
        {granted
          ? <CheckCircle2 key="granted" className="size-[18px] anim-check-pop" aria-hidden />
          : <Icon className="size-[18px]" strokeWidth={1.75} aria-hidden />}
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-[13px] font-medium text-app-text tracking-[-0.005em]">
            {t(`onboarding.item.${permKey}`)}
          </span>
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

/* ---------------------------- Step 1: API key ----------------------------- */

interface ApiKeyStepProps { hasApiKey: boolean; onSaved: () => void; }

function ApiKeyStep({ hasApiKey, onSaved }: ApiKeyStepProps) {
  const { t } = useTranslation();
  if (hasApiKey) {
    return (
      <section className="anim-fade-up max-w-md mx-auto">
        <div className="text-center">
          <div className="mx-auto size-14 rounded-full bg-app-success-soft grid place-items-center mb-5 shine-sm">
            <CheckCircle2 className="size-7 text-app-success anim-check-pop" aria-hidden />
          </div>
          <h2 className="text-display-md text-app-text">{t('onboarding.ready.title')}</h2>
          <p className="mt-2 text-[13px] text-app-muted leading-relaxed">{t('onboarding.ready.subtitle')}</p>
        </div>

        {/* What the user has — all of it, permanently. This screen used to
            announce a 4-day Pro trial, caps and a paywall that the product no
            longer has. No purchase is mentioned here at all: per
            docs/ttp-pro-design.md there is exactly one mention, in Settings. */}
        <div className="mt-7 rounded-app-lg border border-app-border bg-app-surface shine-sm p-5">
          <div className="flex items-center gap-2 mb-3">
            <Sparkles className="size-4 text-app-accent" aria-hidden />
            <span className="text-[12px] font-semibold text-app-text uppercase tracking-wide">
              {t('onboarding.ready.includedHeader')}
            </span>
          </div>
          <ul className="space-y-2.5">
            {[
              t('onboarding.ready.perkPolish'),
              t('onboarding.ready.perkDictionary'),
              t('onboarding.ready.perkHistory'),
            ].map((perk, i) => (
              <li key={i} className="flex items-start gap-2.5 text-[13px] text-app-text">
                <CheckCircle2 className="size-3.5 mt-0.5 shrink-0 text-app-success" aria-hidden />
                <span>{perk}</span>
              </li>
            ))}
          </ul>
        </div>
      </section>
    );
  }

  return (
    <section className="anim-fade-up">
      <h2 className="text-display-sm text-app-text">{t('onboarding.wizard.apiKeyTitle')}</h2>
      <p className="mt-2 text-[13px] text-app-muted leading-relaxed">{t('onboarding.wizard.apiKeySubtitle')}</p>
      <div className="mt-7">
        <ApiKeyForm onSuccess={onSaved} submitLabel={t('onboarding.wizard.next')} />
      </div>
    </section>
  );
}

/* ------------------------------ Step 2: Try it ---------------------------- */

/**
 * A real dictation, not a tour. The text box is in this window, so TTP pastes
 * into itself and the user watches their own words arrive — the one proof
 * that permissions, key and trigger are all right.
 */
/* ------------------------- Step 2: Screen context ------------------------- */

/**
 * An explicit yes or no, on its own page, before the first dictation.
 *
 * Reading the screen sends some of what is on it to a third party, so it is
 * off until the user chooses it here (the setting defaults to false) and the
 * page says what goes, where, and what never does. There is no Next button on
 * this step: one of the two answers is the way forward. Amir, 2026-09-15:
 * "on leur demande quand même s'ils veulent ou pas, et on leur explique".
 */
function ScreenStep({ onChosen }: { onChosen: () => void }) {
  const { t } = useTranslation();
  const [saving, setSaving] = useState(false);

  const choose = async (enabled: boolean) => {
    setSaving(true);
    try { await invoke('set_settings', { settings: { screen_context_enabled: enabled } }); }
    catch (e) { console.error('Failed to save screen_context_enabled:', e); }
    finally { setSaving(false); onChosen(); }
  };

  return (
    <section className="anim-fade-up">
      <div className="mb-5 size-10 rounded-app-md grid place-items-center bg-app-accent-tint text-app-accent">
        <ScanText className="size-5" strokeWidth={1.75} aria-hidden />
      </div>
      <h2 className="text-display-sm text-app-text">{t('onboarding.screen.title')}</h2>
      <p className="mt-2 text-[13px] text-app-muted leading-relaxed">{t('onboarding.screen.subtitle')}</p>

      <Card elevation="sm" className="mt-6 px-5 py-4">
        <p className="text-[12px] text-app-muted">{t('onboarding.screen.exampleSaid')}</p>
        <p className="mt-1 text-[13px] text-app-text">{t('onboarding.screen.exampleResult')}</p>
      </Card>

      <ul className="mt-5 space-y-2.5 text-[12.5px] leading-relaxed">
        <li className="text-app-text">
          <span className="font-medium">{t('onboarding.screen.sentLabel')}</span> {t('onboarding.screen.sentBody')}
        </li>
        <li className="flex gap-2 text-app-text">
          <ShieldCheck className="size-4 mt-0.5 shrink-0 text-app-success" aria-hidden />
          <span>{t('onboarding.screen.never')}</span>
        </li>
        <li className="text-app-muted">{t('onboarding.screen.later')}</li>
      </ul>

      <div className="mt-7 flex flex-wrap gap-2">
        <Button size="md" onClick={() => choose(true)} disabled={saving}>
          {t('onboarding.screen.yes')}
        </Button>
        <Button size="md" variant="secondary" onClick={() => choose(false)} disabled={saving}>
          {t('onboarding.screen.no')}
        </Button>
      </div>
    </section>
  );
}

/* ------------------------------ Step 3: Try ------------------------------- */

function TryStep() {
  const { t } = useTranslation();
  const { label } = useTrigger();
  const [worked, setWorked] = useState(false);
  // Asked once, here, and off until the user says yes. Sent as a one-key
  // payload: `set_settings` merges it, and this window's store was never
  // loaded, so a full save from it would reset everything else.
  const [crashReports, setCrashReports] = useState(false);
  const handleCrashReports = async (enabled: boolean) => {
    setCrashReports(enabled);
    try { await invoke('set_settings', { settings: { telemetry_enabled: enabled } }); }
    catch (e) { console.error('Failed to save telemetry_enabled:', e); setCrashReports(!enabled); }
  };

  useTauriEvent<{ stage: string }>('transcription-progress', (event) => {
    if (event.payload.stage === 'complete') setWorked(true);
  });

  return (
    <section className="anim-fade-up">
      <h2 className="text-display-sm text-app-text">{t('onboarding.try.title')}</h2>
      <p className="mt-2 text-[13px] text-app-muted leading-relaxed">
        {t('onboarding.try.subtitle', { key: label || (IS_MAC ? 'fn' : 'Ctrl+Space') })}
      </p>

      {IS_MAC && (
        <div className="mt-6">
          <TriggerPicker />
        </div>
      )}

      <textarea
        autoFocus
        rows={4}
        placeholder={t('onboarding.try.placeholder')}
        className="mt-5 w-full resize-none rounded-app-md border border-app-border bg-app-surface px-3.5 py-3 text-[13px] text-app-text placeholder:text-app-faint focus:border-app-accent focus:outline-none"
      />

      {worked ? (
        <p className="mt-4 flex items-start gap-2 text-[13px] font-medium text-app-success">
          <CheckCircle2 className="size-4 mt-0.5 shrink-0" aria-hidden />
          {t('onboarding.try.worked')}
        </p>
      ) : (
        <p className="mt-4 text-[12px] text-app-muted leading-relaxed">{t('onboarding.try.menuBar')}</p>
      )}

      <div className="mt-6 flex items-start justify-between gap-4 border-t border-app-border pt-4">
        <div>
          <p className="text-[13px] font-medium text-app-text">{t('onboarding.try.crashLabel')}</p>
          <p className="mt-0.5 text-[12px] text-app-muted leading-relaxed">{t('onboarding.try.crashDesc')}</p>
        </div>
        <Toggle
          size="sm"
          enabled={crashReports}
          onChange={handleCrashReports}
          aria-label={t('onboarding.try.crashLabel')}
        />
      </div>
    </section>
  );
}

/* ---------------------------- Footer dots ------------------------------- */

interface StepDotsProps { current: number; total: number; }

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
              'relative size-1.5 rounded-full transition-colors duration-hover ease-app-out',
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
