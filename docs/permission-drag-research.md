# Drag-to-authorize permissions — research

Research for the onboarding feature Amir asked for on 2026-09-14: grant a
permission by dragging TTP's icon into the System Settings list, the way the
Codex app does. Nothing here is implemented yet.

[V] = verified in source code, a binary on this Mac, or on macOS 26.6.2.
[I] = inferred or claimed by others, not tested.

## What Codex does

- A borderless, non-activating panel (~530×109 pt, frosted, radius 18) sits
  inside the bottom edge of the System Settings content area and follows the
  window. It shows an up-arrow, "Drag Codex to the list above to allow
  Accessibility", and a row with the app icon that is the drag source. [V class
  names and strings in the `Codex Computer Use.app` binary; layout from
  zats/permiso, reverse-engineered, I]
- Covers Accessibility and Screen Recording. [V]

## Mechanism

- The row starts a native drag session carrying the `.app` bundle's file URL,
  app icon as drag image. PermissionFlow also adds `NSFilenamesPboardType`,
  `public.url` and the plain path, saying Settings accepts it "more reliably"
  when it looks like a Finder drag. While dragging, the panel ignores the mouse
  so it cannot block the drop.
- Positioning: `CGWindowListCopyWindowInfo` filtered on the
  `com.apple.systempreferences` pid, layer 0, largest window. Bounds are
  readable without Screen Recording permission. Poll 30–150 ms; hide when
  Settings is not frontmost.
- Deep links TTP already uses still work on 26.6.2 [V]:
  `x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility`,
  `Privacy_ListenEvent`, `Privacy_Microphone`.
- **Microphone: not possible.** That list has no "+" and only lists apps that
  asked. Keep the system prompt. [V PermissionFlow excludes it]
- **Input Monitoring: works** [V PermissionFlow supports it]. macOS then offers
  "Quit & Reopen" and `CGPreflightListenEventAccess` may stay false until
  relaunch [I], so onboarding needs a restart step.
- **Does a drop add the row switched on?** Sources disagree [I]. If a stale row
  for TTP already exists, the drop probably does nothing. Needs a manual test
  on 26.x before relying on it.

## Doing it in Tauri 2

- `tauri-plugin-drag` 2.1.1 (crabnebula `drag-rs`) starts a native file drag
  from a webview [V source]. Caveat: it only provides `public.file-url` and
  builds the URL with `isDirectory:false`. If Settings refuses the drop, write
  a ~100-line objc2 drag source copying PermissionFlow's
  `AppBundlePasteboardWriter`.
- Helper window: `transparent`, no decorations, always on top, `focused(false)`,
  `accept_first_mouse(true)` (otherwise the first click only focuses). If
  clicking it pushes Settings back, use `tauri-nspanel` for a non-activating
  panel. Track the Settings window from a Rust thread with the
  `core-graphics` crate TTP already depends on.
- Detect the grant by polling `AXIsProcessTrusted` / `probe_accessibility` and
  `CGPreflightListenEventAccess`; close the helper and advance onboarding.

## Pitfalls specific to TTP

- `tauri dev` runs a bare binary, so TCC grants the parent terminal. Only show
  the helper when running from a `.app`.
- Ad-hoc signing (`"signingIdentity": "-"`) ties grants to the exact build: an
  update looks like a new app, Settings shows TTP enabled while
  `AXIsProcessTrusted` is false. Reset stale trust first
  (`reset_accessibility_tcc`); the real fix is Developer ID signing.
- App translocation or a DMG path: tell the user to move TTP to Applications.
- The onboarding window is always-on-top and would cover System Settings.

## References

- zats/permiso — Codex clone, Swift, no licence (not reusable)
- jaywcjlove/PermissionFlow — MIT, Swift, most complete
- pingdotgg/t3code PR #11289 — MIT, Electron webview panel, closest analogue
- crabnebula-dev/drag-rs (`tauri-plugin-drag`) — Apache-2.0/MIT
- ahkohd/tauri-nspanel — Apache-2.0
