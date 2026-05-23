import type { HTMLAttributes, ReactNode } from 'react';
import { cn } from '../../lib/cn';

interface DarkPillProps extends HTMLAttributes<HTMLDivElement> {
  tone?: 'idle' | 'active' | 'danger';
  children: ReactNode;
}

/**
 * Dark overlay pill used by the floating bar and tutorial hint. Black tinted
 * surface, ring highlight, blur, layered ambient + contact shadow with an
 * inset top highlight so it reads as molded glass rather than a flat
 * rectangle. The recording pill and the tutorial pill both need this exact
 * chrome — extract once, use everywhere.
 *
 * Theme-agnostic by design: works against any wallpaper.
 */
export function DarkPill({ tone = 'active', children, className, ...rest }: DarkPillProps) {
  const tones = {
    idle: 'bg-black/55 ring-1 ring-white/5',
    active: 'bg-black/90 ring-1 ring-white/10',
    danger: 'bg-app-danger/95 ring-1 ring-white/10',
  };
  return (
    <div
      className={cn(
        'rounded-full backdrop-blur-md',
        'shadow-[0_2px_4px_rgba(0,0,0,0.4),0_12px_32px_rgba(0,0,0,0.5),inset_0_1px_0_rgba(255,255,255,0.08)]',
        tones[tone],
        className,
      )}
      {...rest}
    >
      {children}
    </div>
  );
}
