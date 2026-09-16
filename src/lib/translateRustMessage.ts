// TTP - Talk To Paste
// Shared helper to resolve Rust-side translation keys via i18next.
//
// The pipeline (and a few standalone Tauri commands) emit translation keys
// like `error.no_speech`, `progress.transcribing`, `permission.input_monitoring_required`.
// Callers pass them through this helper before rendering. Anything that
// doesn't look like a key (no namespace prefix) is returned verbatim so an
// older Rust version that still emits literal text doesn't show up as
// "missing key" placeholder copy in the UI.

const KEY_PREFIXES = ['error.', 'progress.', 'permission.'] as const;

export type Translator = (
  key: string,
  params?: Record<string, string | number>,
) => string;

export function translateRustMessage(
  t: Translator,
  message: string,
  params?: Record<string, string | number>,
): string {
  if (!message) return '';
  if (message.includes('.') && KEY_PREFIXES.some((p) => message.startsWith(p))) {
    return t(message, params);
  }
  return message;
}
