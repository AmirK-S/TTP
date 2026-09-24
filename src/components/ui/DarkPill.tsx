import type { HTMLAttributes, ReactNode } from 'react';
import { cn } from '../../lib/cn';

interface DarkPillProps extends HTMLAttributes<HTMLDivElement> {
  tone?: 'idle' | 'active' | 'danger';
  children: ReactNode;
}

/**
 * Dark overlay pill used by the floating bar and tutorial hint. Tinted
 * surface, ring highlight, blur, layered ambient + contact shadow with an
 * inset top highlight so it reads as molded glass rather than a flat
 * rectangle. The recording pill and the tutorial pill both need this exact
 * chrome — extract once, use everywhere.
 *
 * **Still appearance-agnostic, now coat-aware.** The pill floats over the
 * user's wallpaper rather than over an app canvas, so it stays dark whether
 * macOS is in light mode or dark — that has not changed and must not. What it
 * now takes from the coat is *which* dark: `--ttp-pill` and `--ttp-pill-ring`
 * (src/styles/coats.css). The free coat's values reproduce the previous
 * `bg-black/90` and `ring-white/10` exactly, so nothing moved for anybody who
 * has not chosen otherwise.
 *
 * The ring folds into the box-shadow rather than staying a `ring-1` utility so
 * that its colour is a token like everything else in the stack instead of a
 * Tailwind ring variable the coat layer cannot reach.
 */
export function DarkPill({ tone = 'active', children, className, style, ...rest }: DarkPillProps) {
  /** How much of the coat's pill colour to lay down. Idle is half-there. */
  const opacity = tone === 'idle' ? '55%' : '90%';
  const ring =
    tone === 'idle'
      ? 'color-mix(in srgb, var(--ttp-pill-ring) 50%, transparent)'
      : 'var(--ttp-pill-ring)';
  return (
    <div
      className={cn(
        'rounded-full backdrop-blur-md',
        tone === 'danger' && 'bg-app-danger/95',
        className,
      )}
      style={{
        ...(tone === 'danger'
          ? {}
          : { background: `color-mix(in srgb, var(--ttp-pill) ${opacity}, transparent)` }),
        boxShadow: [
          '0 2px 4px rgba(0,0,0,0.4)',
          '0 12px 32px rgba(0,0,0,0.5)',
          'inset 0 1px 0 rgba(255,255,255,0.08)',
          `inset 0 0 0 1px ${ring}`,
        ].join(', '),
        ...(style ?? {}),
      }}
      {...rest}
    >
      {children}
    </div>
  );
}
