import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';

interface RadioOptionProps {
  selected: boolean;
  onSelect: () => void;
  label: ReactNode;
  description?: ReactNode;
  trailing?: ReactNode;
  disabled?: boolean;
}

/**
 * Used for shortcut chooser, language picker, etc. Replaces the inline
 * border-2 transition-all radio buttons that jitter their neighbors on
 * selection. Uses outline-offset trickery to avoid the 2px width shift —
 * a senior-designer detail.
 */
export function RadioOption({ selected, onSelect, label, description, trailing, disabled }: RadioOptionProps) {
  return (
    <button
      type="button"
      onClick={onSelect}
      disabled={disabled}
      className={cn(
        'ttp-radio group w-full flex items-center justify-between gap-3 px-4 py-3 text-left',
        'rounded-app-md border border-app-border bg-app-surface',
        'transition-[background-color,border-color,box-shadow] duration-hover ease-app-out',
        'hover:bg-app-raised hover:border-app-border-strong',
        'active:scale-[0.995] active:duration-press',
        selected && 'border-app-accent bg-app-accent-tint shadow-[inset_0_0_0_1px_var(--accent)]',
        disabled && 'opacity-50 cursor-not-allowed',
      )}
      aria-pressed={selected}
    >
      <span className="flex items-center gap-3 min-w-0">
        <span
          className={cn(
            'size-4 rounded-full border-2 grid place-items-center shrink-0 transition-colors duration-hover',
            selected ? 'border-app-accent' : 'border-app-border-strong',
          )}
          aria-hidden
        >
          {selected && (
            <span className="size-2 rounded-full bg-app-accent anim-check-pop" />
          )}
        </span>
        <span className={cn('min-w-0 text-[13px]', selected ? 'text-app-accent font-medium' : 'text-app-text')}>
          {label}
        </span>
      </span>
      {trailing && <span className="text-[12px] text-app-faint shrink-0">{trailing}</span>}
      {!trailing && description && (
        <span className="text-[12px] text-app-faint shrink-0">{description}</span>
      )}
    </button>
  );
}
