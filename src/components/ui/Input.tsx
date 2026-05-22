import { forwardRef, type InputHTMLAttributes, type ReactNode } from 'react';
import { cn } from '../../lib/cn';

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  invalid?: boolean;
  leftIcon?: ReactNode;
  rightSlot?: ReactNode;
}

/**
 * Border shifts to accent on focus, shadow gets a subtle accent-tinted
 * glow. Matches the Linear / Raycast pattern — no thick ring outline,
 * just a colour shift + small tinted halo.
 */
export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { invalid, leftIcon, rightSlot, className, ...rest },
  ref,
) {
  return (
    <div
      className={cn(
        'flex items-center h-9 rounded-app-md border bg-app-surface text-sm text-app-text',
        'transition-[border-color,box-shadow] duration-150 ease-[cubic-bezier(0.32,0.72,0,1)]',
        invalid
          ? 'border-app-danger focus-within:shadow-[0_0_0_3px_var(--danger-soft)]'
          : 'border-app-border focus-within:border-app-accent focus-within:shadow-[0_0_0_3px_var(--accent-soft)]',
        className,
      )}
    >
      {leftIcon && (
        <span className="pl-3 pr-1.5 text-app-faint inline-flex items-center [&_svg]:size-3.5">
          {leftIcon}
        </span>
      )}
      <input
        ref={ref}
        className={cn(
          'flex-1 min-w-0 h-full bg-transparent outline-none border-0 px-3 placeholder:text-app-faint',
          leftIcon ? 'pl-0' : '',
          rightSlot ? 'pr-1' : '',
        )}
        {...rest}
      />
      {rightSlot && <span className="pr-2 inline-flex items-center">{rightSlot}</span>}
    </div>
  );
});
