import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';

interface EmptyStateProps {
  icon: ReactNode;
  title: string;
  description?: string;
  action?: ReactNode;
  className?: string;
}

/**
 * Empty-state pattern used in History, Dictionary, etc. A token-tinted icon
 * tile, a tight headline, and a muted explanation. The user should never see
 * "Nothing here." as a centered grey sentence — that's the AI-tell. This is
 * the antidote.
 */
export function EmptyState({ icon, title, description, action, className }: EmptyStateProps) {
  return (
    <div className={cn('flex flex-col items-center text-center px-6 py-10', className)}>
      <div className="size-12 rounded-app-lg bg-app-raised border border-app-border grid place-items-center text-app-faint shine-sm mb-4 [&_svg]:size-5">
        {icon}
      </div>
      <p className="text-display-xs text-app-text">{title}</p>
      {description && (
        <p className="mt-1.5 text-[13px] text-app-muted max-w-xs leading-relaxed">{description}</p>
      )}
      {action && <div className="mt-4">{action}</div>}
    </div>
  );
}
