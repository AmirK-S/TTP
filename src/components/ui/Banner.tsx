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
    wrap: 'bg-app-accent-tint border-app-accent/25',
    icon: <Info className="size-4 text-app-accent" aria-hidden />,
  },
  warning: {
    wrap: 'bg-app-warning-tint border-app-warning/30',
    icon: <AlertTriangle className="size-4 text-app-warning" aria-hidden />,
  },
  success: {
    wrap: 'bg-app-success-tint border-app-success/25',
    icon: <CheckCircle2 className="size-4 text-app-success" aria-hidden />,
  },
  danger: {
    wrap: 'bg-app-danger-tint border-app-danger/30',
    icon: <XCircle className="size-4 text-app-danger" aria-hidden />,
  },
};

export function Banner({ tone = 'info', title, children, action, className }: BannerProps) {
  const t = tones[tone];
  return (
    <div
      role={tone === 'danger' || tone === 'warning' ? 'alert' : 'status'}
      className={cn(
        'flex items-start gap-3 px-4 py-3 rounded-app-md border shine',
        t.wrap,
        className,
      )}
    >
      <span className="mt-0.5 shrink-0">{t.icon}</span>
      <div className="flex-1 min-w-0 text-[13px]">
        {title && <div className="font-medium text-app-text">{title}</div>}
        {children && <div className={cn(title && 'mt-0.5', 'text-app-muted')}>{children}</div>}
      </div>
      {action && <div className="shrink-0">{action}</div>}
    </div>
  );
}
