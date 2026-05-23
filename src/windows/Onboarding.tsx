// TTP - Talk To Paste
// First-launch onboarding wizard.
//
// Five steps: Welcome → Permissions → API key → Preferences → Tour. Gating is
// intentionally soft: only Microphone and the API key are required to advance.
// Accessibility + Input Monitoring are strong recommendations but skippable.
// Preferences default to the safe choices; Tour is purely informational.

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useTranslation } from 'react-i18next';
import {
  enable as enableAutostart,
  disable as disableAutostart,
  isEnabled as isAutostartEnabled,
} from '@tauri-apps/plugin-autostart';
import {
  Mic,
  Accessibility as AccessibilityIcon,
  Keyboard,
  CheckCircle2,
  ChevronRight,
  ChevronLeft,
  ExternalLink,
  Sparkles,
  Search,
  EyeOff,
  Power,
  Bug,
} from 'lucide-react';
import { Button, Card, BrandTile, Toggle, DarkPill } from '../components/ui';
import { ApiKeyForm } from '../components/ApiKeyForm';
import { cn } from '../lib/cn';

type PermissionStatus = 'Granted' | 'Denied' | 'Undetermined';
type PermKey = 'microphone' | 'accessibility' | 'inputMonitoring';
type StepIndex = 0 | 1 | 2 | 3 | 4;

const TOTAL_STEPS = 5;
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
    return n >= 0 && n <= 4 ? (n as StepIndex) : 0;
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
    if (step !== 1) return;
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
  const advance = () => setStep((s) => Math.min(4, s + 1) as StepIndex);
  const back = () => setStep((s) => Math.max(0, s - 1) as StepIndex);

  return (
    <div className="min-h-screen flex flex-col bg-app-bg bg-noise">
      <div className="flex-1 overflow-y-auto">
        {/* `key={step}` forces a remount so the anim-fade-up entry replays.
            A poor man's crossfade without framer-motion. */}
        <div key={step} className="max-w-xl mx-auto px-8 pt-16 pb-8">
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
            <ApiKeyStep hasApiKey={hasApiKey} onSaved={() => setHasApiKey(true)} />
          )}
          {step === 3 && <PreferencesStep />}
          {step === 4 && <TourStep />}
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
              <Button size="md" rightIcon={<ChevronRight className="size-4" />} onClick={advance}>
                {t('onboarding.wizard.next')}
              </Button>
            )}
            {step === 1 && (
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
            {step === 2 && hasApiKey && (
              <Button size="md" rightIcon={<ChevronRight className="size-4" />} onClick={advance}>
                {t('onboarding.wizard.next')}
              </Button>
            )}
            {step === 3 && (
              <Button size="md" rightIcon={<ChevronRight className="size-4" />} onClick={advance}>
                {t('onboarding.wizard.next')}
              </Button>
            )}
            {step === 4 && (
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
      <BrandTile size="lg" className="mx-auto mb-7" />
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
  const items: PermKey[] = IS_MAC ? ['microphone', 'accessibility', 'inputMonitoring'] : ['microphone'];

  return (
    <section className="anim-fade-up">
      <h2 className="text-display-sm text-app-text">{t('onboarding.wizard.permissionsTitle')}</h2>
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

/* ---------------------------- Trial countdown ----------------------------- */

function TrialCountdown() {
  const { t } = useTranslation();
  const [trialStartedAt, setTrialStartedAt] = useState<number | null>(null);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    invoke<{ trial_started_at: number | null }>('get_usage_stats')
      .then((u) => setTrialStartedAt(u.trial_started_at))
      .catch(() => {});
  }, []);

  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 60_000);
    return () => window.clearInterval(id);
  }, []);

  if (!trialStartedAt) return null;
  const endMs = (trialStartedAt + 7 * 86_400) * 1000;
  const msLeft = Math.max(0, endMs - now);
  if (msLeft === 0) return null;
  const days = Math.floor(msLeft / 86_400_000);
  const hours = Math.floor((msLeft % 86_400_000) / 3_600_000);
  const minutes = Math.floor((msLeft % 3_600_000) / 60_000);

  return (
    <div className="mt-3 inline-flex items-center gap-2 px-3 py-1 rounded-full bg-app-accent-tint border border-app-accent/20">
      <span className="size-1.5 rounded-full bg-app-accent anim-pulse" aria-hidden />
      <span className="text-[11px] font-medium text-app-accent tabular-nums">
        {t('onboarding.trial.timeLeft', { days, hours, minutes })}
      </span>
    </div>
  );
}

/* ---------------------------- Step 2: API key ----------------------------- */

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
          <h2 className="text-display-md text-app-text">{t('onboarding.trial.title')}</h2>
          <p className="mt-2 text-[13px] text-app-muted leading-relaxed">{t('onboarding.trial.subtitle')}</p>
          <TrialCountdown />
        </div>

        {/* Trial perks recap. */}
        <div className="mt-7 rounded-app-lg border border-app-border bg-app-surface shine-sm p-5">
          <div className="flex items-center gap-2 mb-3">
            <Sparkles className="size-4 text-app-accent" aria-hidden />
            <span className="text-[12px] font-semibold text-app-text uppercase tracking-wide">
              {t('onboarding.trial.perksHeader')}
            </span>
          </div>
          <ul className="space-y-2.5">
            {[
              t('onboarding.trial.perkPolish'),
              t('onboarding.trial.perkDictionary'),
              t('onboarding.trial.perkHistory'),
            ].map((perk, i) => (
              <li key={i} className="flex items-start gap-2.5 text-[13px] text-app-text">
                <CheckCircle2 className="size-3.5 mt-0.5 shrink-0 text-app-success" aria-hidden />
                <span>{perk}</span>
              </li>
            ))}
          </ul>
        </div>

        <p className="mt-4 text-center text-[11px] text-app-faint">
          {t('onboarding.trial.fallback')}
        </p>

        <a
          href="https://ttp.amirks.eu"
          target="_blank"
          rel="noopener noreferrer"
          className={
            'mt-5 flex items-center justify-between rounded-app-md border border-app-accent/30 ' +
            'bg-app-accent-tint hover:bg-app-accent-soft px-4 py-3 ' +
            'transition-colors duration-hover ease-app-out group'
          }
        >
          <div className="min-w-0">
            <div className="text-[13px] font-medium text-app-text">
              {t('onboarding.trial.upgradeTitle')}
            </div>
            <div className="text-[11px] text-app-muted mt-0.5">
              {t('onboarding.trial.upgradeSubtitle')}
            </div>
          </div>
          <span className="ml-3 inline-flex items-center gap-1 text-[12px] font-medium text-app-accent shrink-0">
            {t('onboarding.trial.upgradeCta')}
            <ExternalLink className="size-3" aria-hidden />
          </span>
        </a>
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

/* -------------------------- Step 3: Preferences --------------------------- */

/**
 * Three opt-in toggles surfaced up-front so users don't have to dig into
 * Settings later. Telemetry default OFF (privacy first), autostart default OFF
 * (don't squat in the user's launchd unless they ask), hide-pill default OFF
 * (visible feedback while they're new to the app — they can hide it later).
 *
 * Each toggle persists immediately; no save button. The pattern matches the
 * Settings panel so muscle-memory transfers.
 */
function PreferencesStep() {
  const { t } = useTranslation();
  const [hidePill, setHidePill] = useState(false);
  const [autostart, setAutostart] = useState(false);
  const [telemetry, setTelemetry] = useState(false);

  // Hydrate from the actual app state so the toggles reflect reality if the
  // user navigates back to this step after changing something.
  useEffect(() => {
    invoke<{ hide_pill_when_inactive?: boolean; telemetry_enabled?: boolean }>('get_settings')
      .then((s) => {
        setHidePill(!!s.hide_pill_when_inactive);
        setTelemetry(!!s.telemetry_enabled);
      })
      .catch(() => {});
    isAutostartEnabled().then(setAutostart).catch(() => {});
  }, []);

  const onHidePill = async (v: boolean) => {
    setHidePill(v);
    try { await invoke('set_settings', { settings: { hide_pill_when_inactive: v } }); }
    catch (e) { console.error('Failed to save hide_pill:', e); setHidePill(!v); }
  };

  const onAutostart = async (v: boolean) => {
    setAutostart(v);
    try { if (v) await enableAutostart(); else await disableAutostart(); }
    catch (e) { console.error('Failed to toggle autostart:', e); setAutostart(!v); }
  };

  const onTelemetry = async (v: boolean) => {
    setTelemetry(v);
    try { await invoke('set_settings', { settings: { telemetry_enabled: v } }); }
    catch (e) { console.error('Failed to save telemetry:', e); setTelemetry(!v); }
  };

  return (
    <section className="anim-fade-up max-w-md mx-auto">
      <h2 className="text-display-sm text-app-text">{t('onboarding.wizard.preferencesTitle')}</h2>
      <p className="mt-2 text-[13px] text-app-muted leading-relaxed">
        {t('onboarding.wizard.preferencesSubtitle')}
      </p>

      <div className="mt-7 space-y-2.5">
        <PrefRow
          icon={<EyeOff className="size-4" strokeWidth={1.75} />}
          tile="bg-app-accent-tint text-app-accent"
          label={t('onboarding.preferences.hidePillLabel')}
          desc={t('onboarding.preferences.hidePillDesc')}
          enabled={hidePill}
          onChange={onHidePill}
          delay={0}
        />
        <PrefRow
          icon={<Power className="size-4" strokeWidth={1.75} />}
          tile="bg-app-success-tint text-app-success"
          label={t('onboarding.preferences.autostartLabel')}
          desc={t('onboarding.preferences.autostartDesc')}
          enabled={autostart}
          onChange={onAutostart}
          delay={1}
        />
        <PrefRow
          icon={<Bug className="size-4" strokeWidth={1.75} />}
          tile="bg-app-warning-tint text-app-warning"
          label={t('onboarding.preferences.telemetryLabel')}
          desc={t('onboarding.preferences.telemetryDesc')}
          enabled={telemetry}
          onChange={onTelemetry}
          delay={2}
        />
      </div>

      <p className="mt-5 text-[11px] text-app-faint text-center">
        {t('onboarding.preferences.footnote')}
      </p>
    </section>
  );
}

function PrefRow({
  icon, tile, label, desc, enabled, onChange, delay,
}: {
  icon: React.ReactNode;
  tile: string;
  label: string;
  desc: string;
  enabled: boolean;
  onChange: (v: boolean) => void;
  delay: number;
}) {
  return (
    <Card
      elevation="sm"
      className="anim-fade-up flex items-center gap-4 px-5 py-4"
      style={{ animationDelay: `${0.06 + delay * 0.05}s` }}
    >
      <div className={cn('shrink-0 size-9 rounded-app-md grid place-items-center', tile)}>
        {icon}
      </div>
      <div className="flex-1 min-w-0">
        <p className="text-[13px] font-medium text-app-text tracking-[-0.005em]">{label}</p>
        <p className="mt-1 text-[12px] text-app-muted leading-snug">{desc}</p>
      </div>
      <Toggle enabled={enabled} onChange={onChange} />
    </Card>
  );
}

/* ----------------------------- Step 4: Tour ------------------------------- */

/**
 * Three illustrated cards walking through the actual usage flow. Replaces the
 * "user finishes the wizard and stares at their desktop wondering what just
 * happened" failure mode. The illustrations are static (an animated kbd glyph,
 * a mini DarkPill mockup, and icons for menu bar + Spotlight) — keeps the
 * bundle tiny and the message clear.
 */
function TourStep() {
  const { t } = useTranslation();
  const shortcutKey = IS_MAC ? 'fn' : 'Ctrl';
  return (
    <section className="anim-fade-up max-w-md mx-auto">
      <h2 className="text-display-sm text-app-text">{t('onboarding.wizard.tourTitle')}</h2>
      <p className="mt-2 text-[13px] text-app-muted leading-relaxed">
        {t('onboarding.wizard.tourSubtitle')}
      </p>

      <div className="mt-7 space-y-3">
        <TourCard
          delay={0}
          illustration={
            <div className="flex items-center justify-center gap-2 px-5 py-4 bg-app-raised border border-app-border rounded-app-md">
              <span className="text-[11px] text-app-muted">{t('onboarding.tour.pressLabel')}</span>
              <kbd className="inline-flex items-center justify-center min-w-[36px] h-[28px] px-2 rounded-app-sm bg-app-surface border border-app-border-strong text-[12px] font-mono font-semibold text-app-text shadow-[inset_0_-1px_0_var(--border-strong)]">
                {shortcutKey.toUpperCase()}
              </kbd>
            </div>
          }
          title={t('onboarding.tour.step1Title')}
          body={t('onboarding.tour.step1Body')}
        />
        <TourCard
          delay={1}
          illustration={
            <div className="flex items-center justify-center px-5 py-4 bg-app-raised border border-app-border rounded-app-md">
              <DarkPill tone="active" className="flex items-center gap-2 px-3 py-1.5">
                <span className="flex items-end gap-[2px] h-[12px]">
                  {[3, 6, 10, 7, 4, 8, 11, 5].map((h, i) => (
                    <span
                      key={i}
                      className="w-[2px] rounded-full bg-white/90 anim-pulse"
                      style={{ height: `${h}px`, animationDelay: `${i * 0.08}s` }}
                    />
                  ))}
                </span>
                <span className="text-[10px] font-medium tabular-nums text-white/90">0:03</span>
              </DarkPill>
            </div>
          }
          title={t('onboarding.tour.step2Title')}
          body={t('onboarding.tour.step2Body')}
        />
        <TourCard
          delay={2}
          illustration={
            <div className="flex items-center justify-center gap-3 px-5 py-4 bg-app-raised border border-app-border rounded-app-md">
              <div className="size-9 rounded-app-sm bg-app-surface border border-app-border grid place-items-center">
                <Mic className="size-4 text-app-accent" strokeWidth={1.75} aria-hidden />
              </div>
              <span className="text-[11px] text-app-muted">{t('onboarding.tour.orLabel')}</span>
              <div className="size-9 rounded-app-sm bg-app-surface border border-app-border grid place-items-center">
                <Search className="size-4 text-app-accent" strokeWidth={1.75} aria-hidden />
              </div>
            </div>
          }
          title={t('onboarding.tour.step3Title')}
          body={t('onboarding.tour.step3Body')}
        />
      </div>

      <p className="mt-6 text-[12px] text-app-success text-center font-medium">
        {t('onboarding.tour.closeReassurance')}
      </p>
    </section>
  );
}

function TourCard({
  illustration, title, body, delay,
}: {
  illustration: React.ReactNode;
  title: string;
  body: string;
  delay: number;
}) {
  return (
    <Card
      elevation="sm"
      className="anim-fade-up p-4"
      style={{ animationDelay: `${0.06 + delay * 0.06}s` }}
    >
      {illustration}
      <p className="mt-3 text-[13px] font-medium text-app-text">{title}</p>
      <p className="mt-1 text-[12px] text-app-muted leading-snug">{body}</p>
    </Card>
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
