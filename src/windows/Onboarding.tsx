// TTP - Talk To Paste
// Onboarding component - checklist flow for permissions and setup

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

/** Permission status from the Rust backend */
type PermissionStatus = 'Granted' | 'Denied' | 'Undetermined';

/** Help text explaining why each step is needed — surfaces motivation upfront. */
const HELP_TEXT: Record<string, string> = {
  microphone: 'Required to capture your voice for transcription.',
  accessibility:
    "Lets TTP paste the transcription into your active app. Without it, text only goes to the clipboard (you'd Cmd+V manually).",
  apikey:
    'Sends audio to Groq Whisper for transcription. Free tier on groq.com is enough — no credit card.',
};

/** Direct deep-links to the right System Settings pane on macOS. */
const SETTINGS_URL: Record<string, string> = {
  microphone: 'x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone',
  accessibility: 'x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility',
};

/**
 * Onboarding window component - shown on first launch
 * Guides user through all setup steps as a checklist
 */
export default function Onboarding() {
  const [checklist, setChecklist] = useState<Record<string, boolean>>({});
  const [permissionStatus, setPermissionStatus] = useState<Record<string, PermissionStatus>>({});
  const [checking, setChecking] = useState<string | null>(null);
  const [apiKeyInput, setApiKeyInput] = useState('');
  const [isSavingKey, setIsSavingKey] = useState(false);
  const [apiKeyError, setApiKeyError] = useState('');

  const checkAllItems = useCallback(async () => {
    try {
      const [micStatus, hasApiKey, accessibilityStatus] = await Promise.all([
        invoke<PermissionStatus>('check_microphone_permission'),
        invoke<boolean>('has_groq_api_key'),
        invoke<PermissionStatus>('check_accessibility_permission'),
      ]);

      setChecklist({
        microphone: micStatus === 'Granted',
        apikey: hasApiKey,
        accessibility: accessibilityStatus === 'Granted',
      });
      setPermissionStatus({
        microphone: micStatus,
        accessibility: accessibilityStatus,
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

  // Open the System Settings pane via the opener plugin (already registered).
  const openSettingsPane = async (key: 'microphone' | 'accessibility') => {
    try {
      await invoke('plugin:opener|open_url', { url: SETTINGS_URL[key] });
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

  // Validate and save API key
  const saveApiKey = async () => {
    if (!apiKeyInput.trim()) {
      setApiKeyError('Enter a valid key');
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
      setApiKeyError(String(e));
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

  const allChecked = checklist.microphone && checklist.apikey && checklist.accessibility;

  // Explicit ordering — Microphone → Accessibility → API key — so the user
  // always knows which step is next instead of guessing.
  const items: Array<{ key: 'microphone' | 'accessibility' | 'apikey'; label: string; }> = [
    { key: 'microphone', label: 'Microphone' },
    { key: 'accessibility', label: 'Accessibility' },
    { key: 'apikey', label: 'Groq API Key' },
  ];

  return (
    <div style={styles.container}>
      <div style={styles.content}>
        <div style={styles.header}>
          <h1 style={styles.title}>Welcome to Talk To Paste</h1>
          <p style={styles.subtitle}>Let's get you set up</p>
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
                        {isChecked ? 'Key saved' : 'No key'}
                      </div>
                      <div style={styles.helpText}>{HELP_TEXT[item.key]}</div>
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
                        {isSavingKey ? 'Validating...' : 'Save'}
                      </button>
                    </div>
                  )}
                  {apiKeyError && <div style={styles.error}>{apiKeyError}</div>}
                </div>
              );
            }

            const onClick = item.key === 'microphone' ? requestMicrophone : requestAccessibility;
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
                    {isChecked ? 'Enabled' : denied ? 'Denied — tap to open Settings' : 'Not enabled'}
                  </div>
                  <div style={styles.helpText}>{HELP_TEXT[item.key]}</div>
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
                    {checking === item.key ? '...' : denied ? 'Open Settings' : 'Enable'}
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
              <div style={styles.trialBannerTitle}>Welcome — 7-day Pro trial just started</div>
              <div style={styles.trialBannerSubtitle}>
                Unlimited polish, dictionary, history. Free tier kicks in after.
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
        >
          {allChecked ? 'Get Started' : 'Complete all steps'}
        </button>

        <p style={styles.hint}>
          You can reopen settings anytime by right-clicking the TTP icon in the menu bar.
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
