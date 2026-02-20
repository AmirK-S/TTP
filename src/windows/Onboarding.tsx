// TTP - Talk To Paste
// Onboarding component - permission check and guidance flow

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

/** Permission status from the Rust backend */
type PermissionStatus = 'Granted' | 'Denied' | 'Undetermined';

interface PermissionInfo {
  status: PermissionStatus;
  message: string;
  instructions: string;
}

/**
 * Onboarding window component - shown on first launch
 * Guides user through microphone permission setup
 */
export default function Onboarding() {
  const [permission, setPermission] = useState<PermissionInfo | null>(null);
  const [isChecking, setIsChecking] = useState(false);
  const [canContinue, setCanContinue] = useState(false);

  // Check permission status on mount
  useEffect(() => {
    checkPermission();
  }, []);

  // Check microphone permission status
  const checkPermission = async () => {
    setIsChecking(true);
    try {
      const status = await invoke<PermissionStatus>('check_microphone_permission');
      
      let message = '';
      let instructions = '';
      
      switch (status) {
        case 'Granted':
          message = 'Microphone access is granted. You\'re all set!';
          instructions = '';
          setCanContinue(true);
          break;
        case 'Denied':
          message = 'Microphone access is denied. Please enable it in System Settings.';
          instructions = '1. Open System Settings\n2. Go to Privacy & Security\n3. Click on Microphone\n4. Enable TTP (Talk To Paste)';
          setCanContinue(false);
          break;
        case 'Undetermined':
          message = 'Microphone permission has not been requested yet.';
          instructions = '1. Open System Settings\n2. Go to Privacy & Security\n3. Click on Microphone\n4. Enable TTP to allow microphone access';
          setCanContinue(false);
          break;
      }
      
      setPermission({ status, message, instructions });
    } catch (error) {
      console.error('Failed to check permission:', error);
      setPermission({
        status: 'Undetermined',
        message: 'Failed to check microphone permission',
        instructions: 'Please check System Settings manually'
      });
    } finally {
      setIsChecking(false);
    }
  };

  // Handle continue button - mark first launch complete and close window
  const handleContinue = async () => {
    try {
      await invoke('mark_first_launch_complete');
      await invoke('close_onboarding');
    } catch (error) {
      console.error('Failed to complete onboarding:', error);
    }
  };

  // Get status indicator color
  const getStatusColor = () => {
    if (!permission) return '#666';
    switch (permission.status) {
      case 'Granted': return '#22c55e';
      case 'Denied': return '#ef4444';
      case 'Undetermined': return '#f59e0b';
    }
  };

  // Get status icon
  const getStatusIcon = () => {
    if (!permission) return '⏳';
    switch (permission.status) {
      case 'Granted': return '✓';
      case 'Denied': return '✕';
      case 'Undetermined': return '?';
    }
  };

  return (
    <div style={styles.container}>
      <div style={styles.content}>
        {/* Header */}
        <div style={styles.header}>
          <h1 style={styles.title}>Welcome to Talk To Paste</h1>
          <p style={styles.subtitle}>
            Voice-powered clipboard - just speak and paste
          </p>
        </div>

        {/* Permission Status Card */}
        <div style={styles.card}>
          <div style={styles.cardHeader}>
            <span style={styles.cardIcon}>🎤</span>
            <h2 style={styles.cardTitle}>Microphone Access</h2>
          </div>
          
          <div style={styles.statusContainer}>
            <div 
              style={{
                ...styles.statusIndicator,
                backgroundColor: getStatusColor()
              }}
            >
              {getStatusIcon()}
            </div>
            <div style={styles.statusText}>
              {permission ? permission.message : 'Checking...'}
            </div>
          </div>

          {/* Instructions for denied/undetermined */}
          {permission && permission.instructions && (
            <div style={styles.instructions}>
              <p style={styles.instructionsTitle}>How to enable:</p>
              <pre style={styles.instructionsText}>{permission.instructions}</pre>
            </div>
          )}

          {/* Re-check button */}
          <button 
            style={styles.checkButton}
            onClick={checkPermission}
            disabled={isChecking}
          >
            {isChecking ? 'Checking...' : 'Check Again'}
          </button>
        </div>

        {/* Continue button */}
        <button
          style={{
            ...styles.continueButton,
            ...(canContinue ? {} : styles.continueButtonDisabled)
          }}
          onClick={handleContinue}
          disabled={!canContinue}
        >
          Continue
        </button>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    minHeight: '100vh',
    backgroundColor: '#1a1a2e',
    color: '#fff',
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    padding: '20px',
  },
  content: {
    width: '100%',
    maxWidth: '420px',
  },
  header: {
    textAlign: 'center',
    marginBottom: '24px',
  },
  title: {
    fontSize: '24px',
    fontWeight: '600',
    margin: '0 0 8px 0',
    color: '#fff',
  },
  subtitle: {
    fontSize: '14px',
    color: '#9ca3af',
    margin: 0,
  },
  card: {
    backgroundColor: '#16213e',
    borderRadius: '12px',
    padding: '20px',
    marginBottom: '20px',
  },
  cardHeader: {
    display: 'flex',
    alignItems: 'center',
    gap: '12px',
    marginBottom: '16px',
  },
  cardIcon: {
    fontSize: '24px',
  },
  cardTitle: {
    fontSize: '18px',
    fontWeight: '600',
    margin: 0,
  },
  statusContainer: {
    display: 'flex',
    alignItems: 'center',
    gap: '12px',
    padding: '12px',
    backgroundColor: '#0f3460',
    borderRadius: '8px',
    marginBottom: '16px',
  },
  statusIndicator: {
    width: '32px',
    height: '32px',
    borderRadius: '50%',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontSize: '16px',
    fontWeight: 'bold',
    color: '#fff',
  },
  statusText: {
    flex: 1,
    fontSize: '14px',
    lineHeight: '1.4',
  },
  instructions: {
    marginBottom: '16px',
  },
  instructionsTitle: {
    fontSize: '13px',
    fontWeight: '600',
    color: '#9ca3af',
    margin: '0 0 8px 0',
  },
  instructionsText: {
    fontSize: '12px',
    color: '#d1d5db',
    backgroundColor: '#0f3460',
    padding: '12px',
    borderRadius: '8px',
    margin: 0,
    whiteSpace: 'pre-line',
    lineHeight: '1.5',
  },
  checkButton: {
    width: '100%',
    padding: '10px 16px',
    backgroundColor: 'transparent',
    border: '1px solid #4b5563',
    borderRadius: '8px',
    color: '#9ca3af',
    fontSize: '14px',
    cursor: 'pointer',
    transition: 'all 0.2s',
  },
  continueButton: {
    width: '100%',
    padding: '14px 24px',
    backgroundColor: '#3b82f6',
    border: 'none',
    borderRadius: '8px',
    color: '#fff',
    fontSize: '16px',
    fontWeight: '600',
    cursor: 'pointer',
    transition: 'all 0.2s',
  },
  continueButtonDisabled: {
    backgroundColor: '#374151',
    color: '#6b7280',
    cursor: 'not-allowed',
  },
};
