import { cn } from '../../lib/cn';

interface BrandTileProps {
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

/**
 * TTP brand mark — rounded surface tile with the wordmark and the accent dot
 * in the top-right. Used in onboarding hero, setup window, settings sidebar.
 * The dot mirrors the app icon's signature blue accent.
 *
 * One source of truth so dot offset, size ratios, and glow stay consistent
 * across surfaces. The legacy duplicates drifted by a pixel here and there;
 * this is the canonical version.
 */
export function BrandTile({ size = 'lg', className }: BrandTileProps) {
  const dim = size === 'sm' ? 'size-7' : size === 'md' ? 'size-10' : 'size-16';
  const text = size === 'sm' ? 'text-[10px]' : size === 'md' ? 'text-[13px]' : 'text-[18px]';
  const dot = size === 'sm' ? 'size-[3px] top-[5px] right-[5px]' : size === 'md' ? 'size-1 top-[7px] right-[7px]' : 'size-1.5 top-[10px] right-[10px]';
  const radius = size === 'sm' ? 'rounded-app-sm' : size === 'md' ? 'rounded-app-md' : 'rounded-app-xl';
  const glow = size === 'lg' ? 'shadow-[0_0_0_3px_rgba(76,139,245,0.18)]' : '';
  return (
    <div
      className={cn(
        'relative bg-app-surface border border-app-border grid place-items-center shine-sm',
        dim,
        radius,
        className,
      )}
    >
      <span className={cn('text-app-text font-semibold tracking-[-0.022em]', text)}>TTP</span>
      <span
        aria-hidden
        className={cn('absolute rounded-full bg-app-accent', dot, glow)}
      />
    </div>
  );
}
