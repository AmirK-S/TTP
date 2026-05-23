import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';

interface SettingsSectionProps {
  id?: string;
  title?: string;
  description?: string;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
  bare?: boolean;
}

/**
 * Section wrapper used throughout Settings. Replaces the
 * `scroll-mt-6 bg-app-surface rounded-app-lg shine-sm border border-app-border p-6 mb-6`
 * boilerplate that was repeated 11 times in the legacy file.
 *
 * `bare` strips the card chrome — use for grouped subsections where the parent
 * already provides the surface.
 */
export function SettingsSection({
  id,
  title,
  description,
  action,
  children,
  className,
  bare,
}: SettingsSectionProps) {
  return (
    <section
      id={id}
      data-section={id}
      className={cn(
        'scroll-mt-6 mb-6',
        !bare && 'bg-app-surface rounded-app-lg shine-sm border border-app-border p-6',
        className,
      )}
    >
      {(title || action) && (
        <div className="flex items-start justify-between gap-4 mb-4">
          <div className="min-w-0">
            {title && (
              <h2 className="text-display-xs text-app-text">{title}</h2>
            )}
            {description && (
              <p className="mt-1 text-[13px] text-app-muted leading-relaxed">{description}</p>
            )}
          </div>
          {action && <div className="shrink-0">{action}</div>}
        </div>
      )}
      {children}
    </section>
  );
}

/**
 * Single setting row: label + description on the left, control on the right.
 * Used for toggles, radio groups, etc. Removes the boilerplate `flex
 * items-center justify-between` markup in each row.
 */
export function SettingsRow({
  label,
  description,
  control,
  className,
}: {
  label: string;
  description?: string;
  control: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn('flex items-center justify-between gap-4 py-2', className)}>
      <div className="min-w-0">
        <p className="text-[13px] font-medium text-app-text">{label}</p>
        {description && (
          <p className="mt-0.5 text-[12px] text-app-muted leading-relaxed">{description}</p>
        )}
      </div>
      <div className="shrink-0">{control}</div>
    </div>
  );
}

/**
 * Subsection divider — used inside a SettingsSection to group related rows.
 */
export function SettingsGroup({
  title,
  children,
  className,
}: {
  title?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn('divide-y divide-app-border', className)}>
      {title && (
        <p className="text-[11px] font-semibold uppercase tracking-[0.06em] text-app-faint pb-2">
          {title}
        </p>
      )}
      {children}
    </div>
  );
}
