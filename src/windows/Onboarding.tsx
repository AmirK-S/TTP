// TTP - Talk To Paste
// Onboarding component - checklist flow for permissions and setup

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

/** Permission status from the Rust backend */
type PermissionStatus = 'Granted' | 'Denied' | 'Undetermined';

/**
 * Onboarding window component - shown on first launch
 * Guides user through all setup steps as a checklist
 */
export default function Onboarding() {
  const [checklist, setChecklist] = useState<Record<string, boolean>>({});
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
    } catch (error) {
      console.error('Failed to check items:', error);
    }
  }, []);

  // Check all permissions/status on mount
  useEffect(() => {
    checkAllItems();
  }, [checkAllItems]);

  // Re-check permissions when window regains focus (user returns from System Settings)
  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) {
        checkAllItems();
      }
    });
    return () => { unlisten.then(fn => fn()); };
  }, [checkAllItems]);

  // Also poll every 2s while window is open (catches changes made in background)
  useEffect(() => {
    const interval = setInterval(checkAllItems, 2000);
    return () => clearInterval(interval);
  }, [checkAllItems]);

  // Request microphone permission - opens System Settings
  const requestMicrophone = async () => {
    setChecking('microphone');
    try {
      await invoke('request_microphone_permission');
    } catch (e) {
      console.log('Microphone permission result:', e);
    } finally {
      setChecking(null);
    }
  };

  // Request accessibility permission - opens System Settings
  const requestAccessibility = async () => {
    setChecking('accessibility');
    try {
      await invoke('request_accessibility_permission');
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

  // Close onboarding and mark first launch complete
  const handleGetStarted = async () => {
    try {
      await invoke('close_onboarding');
    } catch (e) {
      console.error('Failed to close onboarding:', e);
    }
  };

  const allChecked = checklist.microphone && checklist.apikey && checklist.accessibility;

  return (
    <div style={styles.container}>
      <div style={styles.content}>
        {/* Header */}
        <div style={styles.header}>
          <h1 style={styles.title}>Welcome to Talk To Paste</h1>
          <p style={styles.subtitle}>Let's get you set up</p>
        </div>

        {/* Checklist */}
        <div style={styles.checklist}>
          {/* Microphone */}
          <div style={styles.checkItem}>
            <div style={{
              ...styles.statusDot,
              backgroundColor: checklist.microphone ? '#22c55e' : '#ef4444',
            }} />
            <div style={styles.checkContent}>
              <div style={styles.checkLabel}>Microphone</div>
              <div style={{
                ...styles.checkDesc,
                color: checklist.microphone ? '#22c55e' : '#ef4444',
              }}>
                {checklist.microphone ? 'Enabled' : 'Not enabled'}
              </div>
            </div>
            {!checklist.microphone && (
              <button
                style={{
                  ...styles.actionButton,
                  opacity: checking === 'microphone' ? 0.5 : 1,
                }}
                onClick={requestMicrophone}
                disabled={checking === 'microphone'}
              >
                {checking === 'microphone' ? '...' : 'Enable'}
              </button>
            )}
          </div>

          {/* Accessibility */}
          <div style={styles.checkItem}>
            <div style={{
              ...styles.statusDot,
              backgroundColor: checklist.accessibility ? '#22c55e' : '#ef4444',
            }} />
            <div style={styles.checkContent}>
              <div style={styles.checkLabel}>Accessibility</div>
              <div style={{
                ...styles.checkDesc,
                color: checklist.accessibility ? '#22c55e' : '#ef4444',
              }}>
                {checklist.accessibility ? 'Enabled' : 'Not enabled'}
              </div>
            </div>
            {!checklist.accessibility && (
              <button
                style={{
                  ...styles.actionButton,
                  opacity: checking === 'accessibility' ? 0.5 : 1,
                }}
                onClick={requestAccessibility}
                disabled={checking === 'accessibility'}
              >
                {checking === 'accessibility' ? '...' : 'Enable'}
              </button>
            )}
          </div>

          {/* API Key */}
          <div style={styles.checkItemColumn}>
            <div style={styles.checkItemTop}>
              <div style={{
                ...styles.statusDot,
                backgroundColor: checklist.apikey ? '#22c55e' : '#ef4444',
              }} />
              <div style={styles.checkContent}>
                <div style={styles.checkLabel}>Groq API Key</div>
                <div style={{
                  ...styles.checkDesc,
                  color: checklist.apikey ? '#22c55e' : '#ef4444',
                }}>
                  {checklist.apikey ? 'Key saved' : 'No key'}
                </div>
              </div>
            </div>

            {!checklist.apikey && (
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
        </div>

        {/* Continue button */}
        <button
          style={{
            ...styles.continueButton,
            ...(allChecked ? {} : styles.continueButtonDisabled)
          }}
          disabled={!allChecked}
          onClick={handleGetStarted}
        >
          {allChecked ? 'Get Started' : 'Complete all steps'}
        </button>

        {/* Hint about settings access */}
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
    alignItems: 'center',
    gap: '12px',
    backgroundColor: '#141414',
    borderRadius: '10px',
    padding: '14px 16px',
    border: '1px solid #222',
  },
  checkItemColumn: {
    backgroundColor: '#141414',
    borderRadius: '10px',
    padding: '14px 16px',
    border: '1px solid #222',
  },
  checkItemTop: {
    display: 'flex',
    alignItems: 'center',
    gap: '12px',
  },
  statusDot: {
    width: '10px',
    height: '10px',
    borderRadius: '50%',
    flexShrink: 0,
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
  actionButton: {
    padding: '8px 16px',
    backgroundColor: '#2563eb',
    border: 'none',
    borderRadius: '6px',
    color: '#fff',
    fontSize: '12px',
    fontWeight: '600',
    cursor: 'pointer',
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
