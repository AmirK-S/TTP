import type { HTMLAttributes, ReactNode } from 'react';
import { cn } from '../../lib/cn';

interface CardProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  /** Subtle elevation. Use `flat` for sidebar rows etc. */
  elevation?: 'flat' | 'sm' | 'md';
}

const elevations = {
  flat: '',
  sm: 'shadow-app-sm',
  md: 'shadow-app-md',
};

export function Card({ children, elevation = 'sm', className, ...rest }: CardProps) {
  return (
    <div
      className={cn(
        'bg-app-surface border border-app-border rounded-app-md',
        elevations[elevation],
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
    <div className={cn('flex items-start justify-between gap-4 px-5 py-4 border-b border-app-border', className)} {...rest}>
      <div className="min-w-0">
        <h3 className="text-sm font-semibold text-app-text">{title}</h3>
        {description && <p className="mt-1 text-xs text-app-muted">{description}</p>}
      </div>
      {action && <div className="shrink-0">{action}</div>}
    </div>
  );
}

export function CardBody({ children, className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div className={cn('px-5 py-4', className)} {...rest}>
      {children}
    </div>
  );
}
