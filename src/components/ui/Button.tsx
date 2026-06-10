import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from 'react';
import { cn } from '../../lib/cn';

type Variant = 'primary' | 'secondary' | 'ghost' | 'danger';
type Size = 'sm' | 'md' | 'lg';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  loading?: boolean;
  leftIcon?: ReactNode;
  rightIcon?: ReactNode;
  fullWidth?: boolean;
}

/**
 * Hover lifts via brightness, press via scale 0.98 over 80ms — the Apple
 * standard tactile feedback. Transitions enumerate the properties they
 * touch (background-color, transform, color) rather than `transition: all`
 * which is a vibe-coded tell.
 */
const base = cn(
  'inline-flex items-center justify-center gap-2 select-none',
  'font-medium tracking-[-0.005em]',
  'transition-[background-color,color,transform,filter] duration-hover',
  'ease-app-out',
  'active:scale-[0.98] active:duration-press',
  'disabled:cursor-not-allowed disabled:pointer-events-none',
);

const variants: Record<Variant, string> = {
  primary: cn(
    'bg-app-accent text-app-accent-fg shine-sm',
    'hover:brightness-110 hover:bg-app-accent-hover',
    'disabled:bg-app-raised disabled:text-app-faint disabled:shadow-none',
  ),
  secondary: cn(
    'bg-app-surface text-app-text shine-sm border border-app-border',
    'hover:bg-app-raised',
    'disabled:text-app-faint disabled:shadow-none',
  ),
  ghost: cn(
    'bg-transparent text-app-text',
    'hover:bg-app-raised',
    'disabled:text-app-faint',
  ),
  danger: cn(
    'bg-app-danger text-white shine-sm',
    'hover:brightness-110',
    'disabled:bg-app-raised disabled:text-app-faint disabled:shadow-none',
  ),
};

const sizes: Record<Size, string> = {
  sm: 'h-7 px-2.5 text-xs rounded-app-sm',
  md: 'h-8 px-3 text-[13px] rounded-app-md',
  lg: 'h-9 px-4 text-sm rounded-app-md',
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = 'primary', size = 'md', loading, leftIcon, rightIcon, fullWidth, className, children, disabled, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      disabled={disabled || loading}
      className={cn(base, variants[variant], sizes[size], fullWidth && 'w-full', className)}
      {...rest}
    >
      {loading ? (
        <span className="inline-block size-3.5 rounded-full border-2 border-current border-r-transparent animate-spin" aria-hidden />
      ) : (
        leftIcon
      )}
      {children}
      {!loading && rightIcon}
    </button>
  );
});
