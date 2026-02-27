// TTP - Talk To Paste
// Tutorial pill component - old simple style: "Maintiens fn pour dicter"

import { useState, useEffect } from 'react';

interface TutorialPillProps {
  shortcutText?: string;
  _onDismiss?: () => void;
}

/** LocalStorage key for tutorial dismissal */
const TUTORIAL_DISMISSED_KEY = 'tutorial_pill_dismissed';

/**
 * Tutorial pill - simple dark pill showing keyboard shortcut hint
 * Old style: "Maintiens fn pour dicter"
 */
export function TutorialPill({ shortcutText = 'fn' }: TutorialPillProps) {
  const [isVisible, setIsVisible] = useState(false);
  const [isDismissed, setIsDismissed] = useState(true); // Start dismissed, check on mount

  useEffect(() => {
    const dismissed = localStorage.getItem(TUTORIAL_DISMISSED_KEY) === 'true';
    setIsDismissed(dismissed);

    if (!dismissed) {
      const timer = setTimeout(() => setIsVisible(true), 100);
      return () => clearTimeout(timer);
    }
  }, []);

  if (isDismissed) {
    return null;
  }

  return (
    <div
      className="mb-2 flex items-center gap-2 rounded-2xl bg-black/90 px-4 py-2 shadow-xl backdrop-blur-sm"
      style={{
        opacity: isVisible ? 1 : 0,
        transform: isVisible ? 'translateY(0)' : 'translateY(8px)',
        transition: 'all 0.3s ease-out',
        pointerEvents: isVisible ? 'auto' : 'none',
      }}
    >
      <span style={{
        fontSize: '13px',
        fontWeight: '500',
        color: 'rgba(255,255,255,0.8)',
        userSelect: 'none',
      }}>
        Maintiens{' '}
        <span style={{
          display: 'inline-flex',
          alignItems: 'center',
          justifyContent: 'center',
          borderRadius: '4px',
          backgroundColor: 'rgba(255,255,255,0.2)',
          padding: '2px 6px',
          fontSize: '11px',
          fontWeight: '700',
          color: '#fff',
          lineHeight: 1,
        }}>
          {shortcutText.toUpperCase()}
        </span>
        {' '}pour dicter
      </span>
    </div>
  );
}

export default TutorialPill;
