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

const base =
  'inline-flex items-center justify-center gap-2 font-medium transition-colors select-none disabled:opacity-50 disabled:cursor-not-allowed active:scale-[0.98]';

const variants: Record<Variant, string> = {
  primary:
    'bg-app-accent text-app-accent-fg hover:bg-app-accent-hover shadow-app-sm',
  secondary:
    'bg-app-surface text-app-text border border-app-border hover:bg-app-surface-hover',
  ghost:
    'bg-transparent text-app-text hover:bg-app-surface-hover',
  danger:
    'bg-app-danger text-white hover:opacity-90 shadow-app-sm',
};

const sizes: Record<Size, string> = {
  sm: 'h-7 px-2.5 text-xs rounded-app-sm',
  md: 'h-9 px-3.5 text-sm rounded-app-md',
  lg: 'h-11 px-5 text-sm rounded-app-md',
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
