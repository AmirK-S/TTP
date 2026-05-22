import { forwardRef, type InputHTMLAttributes, type ReactNode } from 'react';
import { cn } from '../../lib/cn';

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  invalid?: boolean;
  leftIcon?: ReactNode;
  rightSlot?: ReactNode;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { invalid, leftIcon, rightSlot, className, ...rest },
  ref,
) {
  return (
    <div
      className={cn(
        'flex items-center h-9 rounded-app-md border bg-app-surface text-sm text-app-text',
        'transition-colors',
        invalid
          ? 'border-app-danger focus-within:border-app-danger'
          : 'border-app-border focus-within:border-app-accent',
        className,
      )}
    >
      {leftIcon && <span className="pl-3 text-app-faint inline-flex">{leftIcon}</span>}
      <input
        ref={ref}
        className={cn(
          'flex-1 bg-transparent outline-none border-0 px-3 placeholder:text-app-faint',
          rightSlot ? 'pr-1' : '',
        )}
        {...rest}
      />
      {rightSlot && <span className="pr-2 inline-flex">{rightSlot}</span>}
    </div>
  );
});
