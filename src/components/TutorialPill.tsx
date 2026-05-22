// TTP - Talk To Paste
// Tutorial pill — short hint above the recording pill on first launch.
// Lives inside the transparent floating-bar window so it picks up the same
// dark chrome as the recording pill (system theme is irrelevant here —
// the pill needs to read on any background the user is over).

import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';

interface TutorialPillProps {
  shortcutText?: string;
}

const TUTORIAL_DISMISSED_KEY = 'tutorial_pill_dismissed';

export function TutorialPill({ shortcutText = 'fn' }: TutorialPillProps) {
  const { t } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  const [isDismissed, setIsDismissed] = useState(true);

  useEffect(() => {
    const dismissed = localStorage.getItem(TUTORIAL_DISMISSED_KEY) === 'true';
    setIsDismissed(dismissed);
    if (!dismissed) {
      const timer = setTimeout(() => setIsVisible(true), 100);
      return () => clearTimeout(timer);
    }
  }, []);

  if (isDismissed) return null;

  return (
    <div
      className={
        'mb-2 flex items-center gap-1.5 rounded-full bg-black/90 ring-1 ring-white/10 backdrop-blur-md ' +
        'px-3.5 py-1.5 shadow-[0_8px_24px_rgba(0,0,0,0.4),0_2px_4px_rgba(0,0,0,0.3),inset_0_1px_0_rgba(255,255,255,0.08)] ' +
        'transition-[opacity,transform] duration-300 ease-[cubic-bezier(0.32,0.72,0,1)]'
      }
      style={{
        opacity: isVisible ? 1 : 0,
        transform: isVisible ? 'translateY(0)' : 'translateY(8px)',
        pointerEvents: isVisible ? 'auto' : 'none',
      }}
      role="status"
    >
      <span className="text-[12px] font-medium text-white/85 select-none tracking-[-0.005em]">
        {t('floatingBar.tutorialPrefix')}
      </span>
      <kbd
        className={
          'inline-flex items-center justify-center min-w-[20px] h-[18px] px-1.5 ' +
          'rounded-[5px] bg-white/15 text-[10px] font-semibold text-white/95 ' +
          'shadow-[inset_0_-1px_0_rgba(0,0,0,0.25),inset_0_1px_0_rgba(255,255,255,0.15)] ' +
          'tabular-nums tracking-wide font-mono'
        }
      >
        {shortcutText.toUpperCase()}
      </kbd>
      <span className="text-[12px] font-medium text-white/85 select-none tracking-[-0.005em]">
        {t('floatingBar.tutorialSuffix')}
      </span>
    </div>
  );
}

export default TutorialPill;
