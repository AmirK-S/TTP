import { useEffect, useRef, type ReactNode } from 'react';
import { cn } from '../../lib/cn';
import { Button } from './Button';

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  /** Secondary line shown under the title (smaller, muted). */
  subtitle?: string;
  /**
   * Leading slot rendered to the left of the title block. Typically an icon
   * tile (e.g. WhatsNew's Sparkles inside `bg-app-accent-tint`). When
   * provided, the header layout becomes a flex row instead of a stacked column.
   */
  headerLeading?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  size?: 'sm' | 'md';
  closeOnBackdrop?: boolean;
  /** Caps the modal's vertical size so long content scrolls inside. */
  scrollableContent?: boolean;
}

export function Modal({
  open,
  onClose,
  title,
  subtitle,
  headerLeading,
  children,
  footer,
  size = 'sm',
  closeOnBackdrop = true,
  scrollableContent = false,
}: ModalProps) {
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    panelRef.current?.focus();
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose]);

  if (!open) return null;

  const hasHeader = Boolean(title || subtitle || headerLeading);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-6 bg-black/40 backdrop-blur-md anim-fade-in"
      onClick={closeOnBackdrop ? onClose : undefined}
      role="dialog"
      aria-modal="true"
      aria-labelledby={title ? 'modal-title' : undefined}
    >
      <div
        ref={panelRef}
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
        className={cn(
          'bg-app-surface rounded-app-lg shine-lg border border-app-border anim-modal-in',
          'outline-none flex flex-col',
          size === 'sm' ? 'max-w-sm w-full' : 'max-w-md w-full',
          scrollableContent && 'max-h-[85vh]',
        )}
      >
        {hasHeader && (
          <div className={cn('px-5 pt-5 pb-2 shrink-0', Boolean(headerLeading) && 'flex items-start gap-4')}>
            {headerLeading && <div className="shrink-0">{headerLeading}</div>}
            <div className="min-w-0">
              {title && (
                <h3 id="modal-title" className="text-display-xs text-app-text">
                  {title}
                </h3>
              )}
              {subtitle && (
                <p className="mt-1 text-[12px] text-app-muted">{subtitle}</p>
              )}
            </div>
          </div>
        )}
        <div
          className={cn(
            'px-5 py-4 text-[13px] text-app-muted leading-relaxed',
            scrollableContent && 'overflow-y-auto flex-1 min-h-0',
          )}
        >
          {children}
        </div>
        {footer && (
          <div className="px-5 pb-5 pt-2 flex justify-end gap-2 shrink-0">{footer}</div>
        )}
      </div>
    </div>
  );
}

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  message: string;
  confirmText: string;
  cancelText?: string;
  tone?: 'danger' | 'warning' | 'primary';
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Convenience wrapper over Modal for destructive/warning confirmations.
 * Cancel autofocuses on destructive prompts so a stray Enter doesn't fire
 * the dangerous action.
 */
export function ConfirmDialog({
  open,
  title,
  message,
  confirmText,
  cancelText,
  tone = 'danger',
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const variant = tone === 'danger' ? 'danger' : tone === 'warning' ? 'primary' : 'primary';
  return (
    <Modal
      open={open}
      onClose={onCancel}
      title={title}
      footer={
        <>
          <Button variant="ghost" size="md" onClick={onCancel} autoFocus>
            {cancelText ?? 'Cancel'}
          </Button>
          <Button variant={variant} size="md" onClick={onConfirm}>
            {confirmText}
          </Button>
        </>
      }
    >
      {message}
    </Modal>
  );
}
