// TTP - Talk To Paste
// Tutorial pill component - shows keyboard shortcut hint on first launch

import { useState, useEffect } from 'react';

interface TutorialPillProps {
  /** The shortcut text to display (default: "FN") */
  shortcutText?: string;
  /** Callback when the pill is dismissed */
  onDismiss?: () => void;
}

/** LocalStorage key for tutorial dismissal */
const TUTORIAL_DISMISSED_KEY = 'tutorial_pill_dismissed';

/**
 * Tutorial pill component that shows on first launch.
 * Displays a dismissible tooltip explaining the keyboard shortcut.
 * Position: bottom-center of the screen.
 */
export function TutorialPill({ shortcutText = 'FN', onDismiss }: TutorialPillProps) {
  const [isVisible, setIsVisible] = useState(false);
  const [isDismissed, setIsDismissed] = useState(false);

  useEffect(() => {
    // Check if tutorial was previously dismissed
    const dismissed = localStorage.getItem(TUTORIAL_DISMISSED_KEY) === 'true';
    setIsDismissed(dismissed);
    
    // Only show if not dismissed
    if (!dismissed) {
      // Small delay to let the pill render first
      const timer = setTimeout(() => setIsVisible(true), 100);
      return () => clearTimeout(timer);
    }
  }, []);

  const handleDismiss = () => {
    setIsVisible(false);
    localStorage.setItem(TUTORIAL_DISMISSED_KEY, 'true');
    setIsDismissed(true);
    onDismiss?.();
  };

  if (isDismissed) {
    return null;
  }

  return (
    <div
      className={`
        fixed bottom-4 left-1/2 -translate-x-1/2 z-50
        transition-all duration-300 ease-out
        ${isVisible ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-2 pointer-events-none'}
      `}
    >
      <div className="bg-gray-900 dark:bg-gray-800 text-white px-4 py-2 rounded-full shadow-lg flex items-center gap-3">
        {/* Shortcut indicator */}
        <span className="bg-blue-600-2 py-0.5 rounded px text-xs font-mono font-semibold">
          {shortcutText}
        </span>
        
        {/* Instruction text */}
        <span className="text-sm text-gray-200">
          Press to start recording
        </span>
        
        {/* Dismiss button */}
        <button
          onClick={handleDismiss}
          className="ml-1 text-gray-400 hover:text-white transition-colors"
          aria-label="Dismiss tutorial"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
    </div>
  );
}

export default TutorialPill;
