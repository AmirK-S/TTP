// TTP - Talk To Paste
// The frontend's line into ttp-trace.log.
//
// Until this existed nothing in `src/` wrote to the trace, and docs/tracing.md
// called that the largest remaining hole: an exception between `capture.stop`
// and `dictation.start` left a gap with no reason attached. Records land as
// `ui.<stage>`; the Rust side (`trace_api::trace_ui`) strips `text` fields and
// caps every string, so nothing dictated can ride along.

import { invoke } from '@tauri-apps/api/core';

/** Write `ui.<stage>` to the trace. Never throws, never awaits. */
export function traceUi(stage: string, fields: Record<string, unknown> = {}): void {
  try {
    invoke('trace_ui', { stage: `ui.${stage}`, fields }).catch(() => {});
  } catch {
    /* not in Tauri (tests, dev preview) */
  }
}

/** Report every uncaught error and unhandled rejection in this window. */
export function installUiErrorTrace(windowLabel: string): void {
  window.addEventListener('error', (event) => {
    traceUi('error', {
      window: windowLabel,
      message: String(event.message ?? ''),
      source: (event.filename ?? '').split('/').pop(),
      line: event.lineno,
    });
  });
  window.addEventListener('unhandledrejection', (event) => {
    traceUi('unhandled_rejection', { window: windowLabel, reason: String(event.reason ?? '') });
  });
}
