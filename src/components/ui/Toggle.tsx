import { forwardRef, type ButtonHTMLAttributes } from 'react';
import { cn } from '../../lib/cn';

interface ToggleProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'onChange'> {
  enabled: boolean;
  onChange: (value: boolean) => void;
  size?: 'sm' | 'md';
}

export const Toggle = forwardRef<HTMLButtonElement, ToggleProps>(function Toggle(
  { enabled, onChange, disabled, size = 'md', className, ...rest },
  ref,
) {
  const track = size === 'sm' ? 'h-5 w-9' : 'h-6 w-11';
  const thumb = size === 'sm' ? 'h-4 w-4' : 'h-5 w-5';
  const offset = size === 'sm' ? 'translate-x-4' : 'translate-x-5';
  return (
    <button
      ref={ref}
      type="button"
      role="switch"
      aria-checked={enabled}
      disabled={disabled}
      onClick={() => !disabled && onChange(!enabled)}
      className={cn(
        'relative inline-flex shrink-0 cursor-pointer rounded-full border-2 border-transparent',
        'transition-[background-color] duration-hover ease-app-out',
        'active:scale-[0.96] active:duration-press',
        track,
        enabled ? 'bg-app-accent' : 'bg-app-raised',
        disabled && 'opacity-50 cursor-not-allowed',
        className,
      )}
      {...rest}
    >
      <span
        className={cn(
          'pointer-events-none inline-block transform rounded-full bg-white',
          'shadow-[0_1px_2px_rgba(0,0,0,0.2),0_2px_4px_rgba(0,0,0,0.08)]',
          'ring-0 transition-transform duration-hover ease-app-spring',
          thumb,
          enabled ? offset : 'translate-x-0',
        )}
      />
    </button>
  );
});
