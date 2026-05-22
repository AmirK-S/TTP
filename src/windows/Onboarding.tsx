// TTP - Talk To Paste
// Onboarding component - checklist flow for permissions and setup

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useTranslation } from 'react-i18next';

/** Permission status from the Rust backend */
type PermissionStatus = 'Granted' | 'Denied' | 'Undetermined';

// Help text for each step is sourced from i18n via `onboarding.help.<key>` —
// resolved inline at the call site so we can use the active language at render.

/** macOS-only — TTP doesn't surface this step on Windows. */
const IS_MAC = typeof navigator !== 'undefined' && navigator.platform.startsWith('Mac');

/**
 * Map each onboarding step to the Rust command that opens its System Settings
 * pane. We can't call `plugin:opener|open_url` directly from the WebView for
 * `x-apple.systempreferences:` URLs — the opener plugin's IPC scope rejects
 * non-http(s) schemes, so the click was silently no-op'ing. Going through
 * dedicated Rust commands (which call OpenerExt internally) bypasses that ACL.
 */
const SETTINGS_COMMAND: Record<string, string> = {
  microphone: 'open_microphone_settings',
  accessibility: 'open_accessibility_settings',
  inputMonitoring: 'open_input_monitoring_settings',
};

/**
 * Onboarding window component - shown on first launch
 * Guides user through all setup steps as a checklist
 */
export default function Onboarding() {
  const { t } = useTranslation();
  const [checklist, setChecklist] = useState<Record<string, boolean>>({});
  const [permissionStatus, setPermissionStatus] = useState<Record<string, PermissionStatus>>({});
  const [checking, setChecking] = useState<string | null>(null);
  const [apiKeyInput, setApiKeyInput] = useState('');
  const [isSavingKey, setIsSavingKey] = useState(false);
  const [apiKeyError, setApiKeyError] = useState('');

  // Keep the window title in sync with the active language.
  useEffect(() => {
    getCurrentWindow().setTitle(t('windowTitle.onboarding'));
  });

  const checkAllItems = useCallback(async () => {
    try {
      // Input Monitoring is mac-only; on Windows we treat it as already passed
      // so the rest of the checklist gating logic is uniform.
      const inputMonitoringPromise = IS_MAC
        ? invoke<boolean>('check_input_monitoring_permission')
        : Promise.resolve(true);

      const [micStatus, hasApiKey, accessibilityStatus, hasInputMonitoring] = await Promise.all([
        invoke<PermissionStatus>('check_microphone_permission'),
        invoke<boolean>('has_groq_api_key'),
        invoke<PermissionStatus>('check_accessibility_permission'),
        inputMonitoringPromise,
      ]);

      setChecklist({
        microphone: micStatus === 'Granted',
        apikey: hasApiKey,
        accessibility: accessibilityStatus === 'Granted',
        inputMonitoring: hasInputMonitoring,
      });
      setPermissionStatus({
        microphone: micStatus,
        accessibility: accessibilityStatus,
        // Input Monitoring doesn't expose Granted/Denied/Undetermined like AVFoundation —
        // the OS just returns a bool. We map it to Granted/Denied for the UI to render
        // the same "tap to enable / tap to open Settings" pattern as the other items.
        inputMonitoring: hasInputMonitoring ? 'Granted' : 'Denied',
      });
    } catch (error) {
      console.error('Failed to check items:', error);
    }
  }, []);

  // Check on mount, then re-check whenever the window regains focus (covers
  // returns from System Settings). 2-second polling was removed — the focus
  // listener catches every realistic case without UI flicker.
  useEffect(() => {
    checkAllItems();
  }, [checkAllItems]);

  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) checkAllItems();
    });
    return () => { unlisten.then(fn => fn()); };
  }, [checkAllItems]);

  const openSettingsPane = async (key: 'microphone' | 'accessibility' | 'inputMonitoring') => {
    try {
      await invoke(SETTINGS_COMMAND[key]);
    } catch (e) {
      console.error('Failed to open settings pane:', e);
    }
  };

  // Request microphone permission - first time triggers the system dialog,
  // already-denied jumps straight to System Settings.
  const requestMicrophone = async () => {
    setChecking('microphone');
    try {
      if (permissionStatus.microphone === 'Denied') {
        await openSettingsPane('microphone');
      } else {
        await invoke('request_microphone_permission');
      }
    } catch (e) {
      console.log('Microphone permission result:', e);
    } finally {
      setChecking(null);
    }
  };

  const requestAccessibility = async () => {
    setChecking('accessibility');
    try {
      if (permissionStatus.accessibility === 'Denied') {
        await openSettingsPane('accessibility');
      } else {
        await invoke('request_accessibility_permission');
      }
    } catch (e) {
      console.log('Accessibility permission result:', e);
    } finally {
      setChecking(null);
    }
  };

  // Input Monitoring: macOS only. The first request triggers the system prompt;
  // any subsequent denial means we have to send the user to System Settings,
  // because macOS won't re-prompt once the user has answered once.
  const requestInputMonitoring = async () => {
    setChecking('inputMonitoring');
    try {
      const granted = await invoke<boolean>('request_input_monitoring_permission');
      if (!granted) {
        // System won't re-prompt — open the right Settings pane so the user
        // can flip the toggle themselves.
        await openSettingsPane('inputMonitoring');
      }
    } catch (e) {
      console.log('Input Monitoring permission result:', e);
    } finally {
      setChecking(null);
    }
  };

  // Validate and save API key
  const saveApiKey = async () => {
    if (!apiKeyInput.trim()) {
      setApiKeyError(t('onboarding.apiKey.enterValid'));
      return;
    }

    setIsSavingKey(true);
    setApiKeyError('');

    try {
      await invoke('validate_groq_api_key', { key: apiKeyInput.trim() });
      await invoke('set_groq_api_key', { key: apiKeyInput.trim() });
      setChecklist(prev => ({ ...prev, apikey: true }));
      setApiKeyInput('');
    } catch (e) {
      // Rust may return a translation key like "error.api_invalid_key";
      // translate those defensively, otherwise show as-is.
      setApiKeyError(
        typeof e === 'string' && (e.startsWith('error.') || e.startsWith('permission.'))
          ? t(e)
          : String(e),
      );
    } finally {
      setIsSavingKey(false);
    }
  };

  const handleGetStarted = async () => {
    try {
      await invoke('close_onboarding');
    } catch (e) {
      console.error('Failed to close onboarding:', e);
    }
  };

  // Input Monitoring only counts on macOS — Windows doesn't have an equivalent
  // permission and we treat it as passed in checkAllItems().
  const allChecked =
    !!checklist.microphone &&
    !!checklist.apikey &&
    !!checklist.accessibility &&
    (!IS_MAC || !!checklist.inputMonitoring);

  // Build a list of which items are still missing — surfaced as a subtitle
  // under the disabled CTA so the user knows what's blocking them.
  const missingLabels: string[] = [];
  if (!checklist.microphone) missingLabels.push(t('onboarding.item.microphone'));
  if (!checklist.accessibility) missingLabels.push(t('onboarding.item.accessibility'));
  if (IS_MAC && !checklist.inputMonitoring) missingLabels.push(t('onboarding.item.inputMonitoring'));
  if (!checklist.apikey) missingLabels.push(t('onboarding.item.apikey'));

  // Explicit ordering — Microphone → Accessibility → Input Monitoring (mac) → API key —
  // so the user always knows which step is next instead of guessing.
  type ItemKey = 'microphone' | 'accessibility' | 'inputMonitoring' | 'apikey';
  const items: Array<{ key: ItemKey; label: string }> = [
    { key: 'microphone', label: t('onboarding.item.microphone') },
    { key: 'accessibility', label: t('onboarding.item.accessibility') },
    ...(IS_MAC ? [{ key: 'inputMonitoring' as const, label: t('onboarding.item.inputMonitoring') }] : []),
    { key: 'apikey', label: t('onboarding.item.apikey') },
  ];

  return (
    <div style={styles.container}>
      <div style={styles.content}>
        <div style={styles.header}>
          <h1 style={styles.title}>{t('onboarding.title')}</h1>
          <p style={styles.subtitle}>{t('onboarding.subtitle')}</p>
        </div>

        <div style={styles.checklist}>
          {items.map((item) => {
            const isChecked = !!checklist[item.key];
            const itemStyle = {
              ...(item.key === 'apikey' ? styles.checkItemColumn : styles.checkItem),
              ...(isChecked ? styles.checkItemCompleted : {}),
            };

            if (item.key === 'apikey') {
              return (
                <div key={item.key} style={itemStyle}>
                  <div style={styles.checkItemTop}>
                    <div style={{
                      ...styles.statusDot,
                      backgroundColor: isChecked ? '#22c55e' : '#ef4444',
                    }} />
                    <div style={styles.checkContent}>
                      <div style={styles.checkLabel}>{item.label}</div>
                      <div style={{
                        ...styles.checkDesc,
                        color: isChecked ? '#22c55e' : '#ef4444',
                      }}>
                        {isChecked ? t('onboarding.apiKey.saved') : t('onboarding.apiKey.missing')}
                      </div>
                      {!isChecked && (
                        <ol style={styles.helpSteps}>
                          <li>
                            {t('onboarding.apiKey.stepSignup')}{' '}
                            <a
                              href="https://console.groq.com"
                              target="_blank"
                              rel="noopener noreferrer"
                              style={styles.helpLink}
                            >
                              console.groq.com
                            </a>
                            {' '}{t('onboarding.apiKey.stepSignupSuffix')}
                          </li>
                          <li>{t('onboarding.apiKey.stepCreate')}</li>
                          <li>{t('onboarding.apiKey.stepCopy')} <code style={styles.helpCode}>gsk_</code> {t('onboarding.apiKey.stepCopySuffix')}</li>
                        </ol>
                      )}
                      <div style={styles.helpText}>{t(`onboarding.help.${item.key}`)}</div>
                    </div>
                  </div>

                  {!isChecked && (
                    <div style={styles.apiKeyInput}>
                      <input
                        type="password"
                        placeholder="gsk_..."
                        value={apiKeyInput}
                        onChange={(e) => setApiKeyInput(e.target.value)}
                        onKeyDown={(e) => e.key === 'Enter' && saveApiKey()}
                        style={styles.input}
                      />
                      <button
                        style={styles.saveButton}
                        onClick={saveApiKey}
                        disabled={isSavingKey}
                      >
                        {isSavingKey ? t('common.validating') : t('common.save')}
                      </button>
                    </div>
                  )}
                  {apiKeyError && <div style={styles.error}>{apiKeyError}</div>}
                </div>
              );
            }

            const onClick =
              item.key === 'microphone'
                ? requestMicrophone
                : item.key === 'accessibility'
                  ? requestAccessibility
                  : requestInputMonitoring;
            const denied = permissionStatus[item.key] === 'Denied';
            return (
              <div key={item.key} style={itemStyle}>
                <div style={{
                  ...styles.statusDot,
                  backgroundColor: isChecked ? '#22c55e' : '#ef4444',
                }} />
                <div style={styles.checkContent}>
                  <div style={styles.checkLabel}>{item.label}</div>
                  <div style={{
                    ...styles.checkDesc,
                    color: isChecked ? '#22c55e' : '#ef4444',
                  }}>
                    {isChecked
                      ? t('onboarding.status.enabled')
                      : denied
                        ? t('onboarding.status.denied')
                        : t('onboarding.status.notEnabled')}
                  </div>
                  <div style={styles.helpText}>{t(`onboarding.help.${item.key}`)}</div>
                </div>
                {!isChecked && (
                  <button
                    style={{
                      ...styles.actionButton,
                      opacity: checking === item.key ? 0.5 : 1,
                    }}
                    onClick={onClick}
                    disabled={checking === item.key}
                  >
                    {checking === item.key
                      ? t('onboarding.button.checking')
                      : denied
                        ? t('onboarding.button.openSettings')
                        : t('onboarding.button.enable')}
                  </button>
                )}
              </div>
            );
          })}
        </div>

        {allChecked && (
          <div style={styles.trialBanner}>
            <div style={styles.trialBannerEmoji}>✨</div>
            <div>
              <div style={styles.trialBannerTitle}>{t('onboarding.trial.title')}</div>
              <div style={styles.trialBannerSubtitle}>
                {t('onboarding.trial.subtitle')}
              </div>
            </div>
          </div>
        )}

        <button
          style={{
            ...styles.continueButton,
            ...(allChecked ? {} : styles.continueButtonDisabled),
          }}
          disabled={!allChecked}
          onClick={handleGetStarted}
          title={
            allChecked
              ? undefined
              : t('onboarding.cta.stillMissing', { items: missingLabels.join(', ') })
          }
        >
          {allChecked ? t('onboarding.cta.getStarted') : t('onboarding.cta.completeSteps')}
        </button>

        {!allChecked && missingLabels.length > 0 && (
          <p style={styles.missingHint}>
            {t('onboarding.cta.stillMissing', { items: missingLabels.join(', ') })}
          </p>
        )}

        <p style={styles.hint}>
          {t('onboarding.cta.reopenHint')}
        </p>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    minHeight: '100vh',
    backgroundColor: '#0a0a0a',
    color: '#fff',
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    padding: '16px',
  },
  content: {
    width: '100%',
    maxWidth: '380px',
  },
  header: {
    textAlign: 'center',
    marginBottom: '20px',
  },
  title: {
    fontSize: '22px',
    fontWeight: '700',
    margin: '0 0 4px 0',
    color: '#fff',
  },
  subtitle: {
    fontSize: '13px',
    color: '#666',
    margin: 0,
  },
  checklist: {
    display: 'flex',
    flexDirection: 'column',
    gap: '10px',
    marginBottom: '20px',
  },
  checkItem: {
    display: 'flex',
    alignItems: 'flex-start',
    gap: '12px',
    backgroundColor: '#141414',
    borderRadius: '10px',
    padding: '14px 16px',
    border: '1px solid #222',
    transition: 'opacity 0.2s',
  },
  checkItemColumn: {
    backgroundColor: '#141414',
    borderRadius: '10px',
    padding: '14px 16px',
    border: '1px solid #222',
    transition: 'opacity 0.2s',
  },
  checkItemCompleted: {
    opacity: 0.55,
  },
  checkItemTop: {
    display: 'flex',
    alignItems: 'flex-start',
    gap: '12px',
  },
  statusDot: {
    width: '10px',
    height: '10px',
    borderRadius: '50%',
    flexShrink: 0,
    marginTop: '4px',
  },
  checkContent: {
    flex: 1,
  },
  checkLabel: {
    fontSize: '14px',
    fontWeight: '600',
    color: '#fff',
    marginBottom: '2px',
  },
  checkDesc: {
    fontSize: '12px',
    fontWeight: '500',
  },
  helpText: {
    fontSize: '11px',
    color: '#888',
    marginTop: '4px',
    lineHeight: 1.4,
  },
  helpSteps: {
    fontSize: '11px',
    color: '#aaa',
    margin: '6px 0 4px 0',
    paddingLeft: '18px',
    lineHeight: 1.5,
  },
  helpLink: {
    color: '#60a5fa',
    textDecoration: 'underline',
  },
  helpCode: {
    fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
    fontSize: '10.5px',
    backgroundColor: '#0a0a0a',
    border: '1px solid #2a2a2a',
    borderRadius: '3px',
    padding: '0 4px',
  },
  missingHint: {
    textAlign: 'center',
    fontSize: '11px',
    color: '#888',
    marginTop: '8px',
    marginBottom: 0,
  },
  actionButton: {
    padding: '8px 16px',
    backgroundColor: '#2563eb',
    border: 'none',
    borderRadius: '6px',
    color: '#fff',
    fontSize: '12px',
    fontWeight: '600',
    cursor: 'pointer',
    flexShrink: 0,
  },
  apiKeyInput: {
    display: 'flex',
    gap: '8px',
    marginTop: '12px',
  },
  input: {
    flex: 1,
    padding: '10px 12px',
    backgroundColor: '#0a0a0a',
    border: '1px solid #333',
    borderRadius: '6px',
    color: '#fff',
    fontSize: '12px',
    outline: 'none',
  },
  saveButton: {
    padding: '10px 16px',
    backgroundColor: '#2563eb',
    border: 'none',
    borderRadius: '6px',
    color: '#fff',
    fontSize: '12px',
    fontWeight: '600',
    cursor: 'pointer',
  },
  error: {
    color: '#ef4444',
    fontSize: '11px',
    marginTop: '6px',
  },
  trialBanner: {
    display: 'flex',
    alignItems: 'center',
    gap: '12px',
    backgroundColor: '#0f2a1a',
    border: '1px solid #1f5d3a',
    borderRadius: '10px',
    padding: '12px 14px',
    marginBottom: '14px',
  },
  trialBannerEmoji: {
    fontSize: '20px',
    flexShrink: 0,
  },
  trialBannerTitle: {
    fontSize: '13px',
    fontWeight: '600',
    color: '#4ade80',
    marginBottom: '2px',
  },
  trialBannerSubtitle: {
    fontSize: '11px',
    color: '#86efac',
  },
  continueButton: {
    width: '100%',
    padding: '16px 20px',
    backgroundColor: '#2563eb',
    border: 'none',
    borderRadius: '10px',
    color: '#fff',
    fontSize: '14px',
    fontWeight: '600',
    cursor: 'pointer',
    transition: 'all 0.2s',
  },
  continueButtonDisabled: {
    backgroundColor: '#222',
    color: '#555',
    cursor: 'not-allowed',
  },
  hint: {
    textAlign: 'center',
    fontSize: '11px',
    color: '#555',
    marginTop: '12px',
  },
};
