import type { ReactNode } from 'react';
import { AlertTriangle, Info, CheckCircle2, XCircle } from 'lucide-react';
import { cn } from '../../lib/cn';

type Tone = 'info' | 'warning' | 'success' | 'danger';

interface BannerProps {
  tone?: Tone;
  title?: string;
  children?: ReactNode;
  action?: ReactNode;
  className?: string;
}

const tones: Record<Tone, { wrap: string; icon: ReactNode }> = {
  info: {
    wrap: 'bg-app-accent-soft text-app-text border-app-accent/30',
    icon: <Info className="size-4 text-app-accent" aria-hidden />,
  },
  warning: {
    wrap: 'bg-app-warning-soft text-app-text border-app-warning/40',
    icon: <AlertTriangle className="size-4 text-app-warning" aria-hidden />,
  },
  success: {
    wrap: 'bg-app-success-soft text-app-text border-app-success/30',
    icon: <CheckCircle2 className="size-4 text-app-success" aria-hidden />,
  },
  danger: {
    wrap: 'bg-app-danger-soft text-app-text border-app-danger/30',
    icon: <XCircle className="size-4 text-app-danger" aria-hidden />,
  },
};

export function Banner({ tone = 'info', title, children, action, className }: BannerProps) {
  const t = tones[tone];
  return (
    <div
      role={tone === 'danger' || tone === 'warning' ? 'alert' : 'status'}
      className={cn(
        'flex items-start gap-3 px-4 py-3 rounded-app-md border',
        t.wrap,
        className,
      )}
    >
      <span className="mt-0.5 shrink-0">{t.icon}</span>
      <div className="flex-1 min-w-0 text-sm">
        {title && <div className="font-medium">{title}</div>}
        {children && <div className={cn(title && 'mt-0.5', 'text-app-muted')}>{children}</div>}
      </div>
      {action && <div className="shrink-0">{action}</div>}
    </div>
  );
}
