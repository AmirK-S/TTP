// TTP - Talk To Paste
// Tutorial pill — short hint above the recording pill on first launch.
// Lives inside the transparent floating-bar window so it picks up the same
// dark chrome as the recording pill (system theme is irrelevant here —
// the pill needs to read on any background the user is over).

import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { DarkPill } from './ui';

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
      const raf = requestAnimationFrame(() => setIsVisible(true));
      return () => cancelAnimationFrame(raf);
    }
  }, []);

  if (isDismissed) return null;

  return (
    <DarkPill
      tone="active"
      className="mb-2 flex items-center gap-1.5 px-3.5 py-1.5 transition-[opacity,transform] duration-modal ease-app-out"
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
          'rounded-app-xs bg-white/15 text-[10px] font-semibold text-white/95 ' +
          'shadow-[inset_0_-1px_0_rgba(0,0,0,0.25),inset_0_1px_0_rgba(255,255,255,0.15)] ' +
          'tabular-nums tracking-wide font-mono'
        }
      >
        {shortcutText.toUpperCase()}
      </kbd>
      <span className="text-[12px] font-medium text-white/85 select-none tracking-[-0.005em]">
        {t('floatingBar.tutorialSuffix')}
      </span>
    </DarkPill>
  );
}

export default TutorialPill;
