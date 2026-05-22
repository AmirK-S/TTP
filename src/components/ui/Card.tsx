import type { HTMLAttributes, ReactNode } from 'react';
import { cn } from '../../lib/cn';

interface CardProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  /**
   * Depth treatment.
   * - `flat`: row inside a list, no lift.
   * - `sm`: default card (subtle shine + shadow).
   * - `md`: emphasised card (deeper shadow, used for hero blocks).
   */
  elevation?: 'flat' | 'sm' | 'md';
  interactive?: boolean;
}

const elevations = {
  flat: 'border border-app-border',
  sm: 'border border-app-border shine-sm',
  md: 'border border-app-border shine-md',
};

export function Card({ children, elevation = 'sm', interactive, className, ...rest }: CardProps) {
  return (
    <div
      className={cn(
        'bg-app-surface rounded-app-lg',
        elevations[elevation],
        interactive && 'transition-colors duration-150 ease-[cubic-bezier(0.32,0.72,0,1)] hover:bg-app-raised',
        className,
      )}
      {...rest}
    >
      {children}
    </div>
  );
}

interface CardHeaderProps extends HTMLAttributes<HTMLDivElement> {
  title: string;
  description?: string;
  action?: ReactNode;
}

export function CardHeader({ title, description, action, className, ...rest }: CardHeaderProps) {
  return (
    <div className={cn('flex items-start justify-between gap-4 px-6 py-4 border-b border-app-border', className)} {...rest}>
      <div className="min-w-0">
        <h3 className="text-[13px] font-semibold tracking-[-0.005em] text-app-text">{title}</h3>
        {description && <p className="mt-1 text-xs text-app-muted leading-relaxed">{description}</p>}
      </div>
      {action && <div className="shrink-0">{action}</div>}
    </div>
  );
}

export function CardBody({ children, className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div className={cn('px-6 py-5', className)} {...rest}>
      {children}
    </div>
  );
}
