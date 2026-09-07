#!/usr/bin/env bash
# Watch the dictation trace for the things that matter, live.
#
# Two weeks of passive harvesting only pays off if someone notices when
# something lands. Leave this running in a terminal and it prints one line per
# anomaly, in colour, with a short gloss of what it means — so a failure is
# noticed the moment it happens, while you still remember what you were doing.
#
#   ./scripts/watch-trace.sh            # follow live
#   ./scripts/watch-trace.sh --since    # replay everything already recorded
#
# The patterns below are deliberately narrow. Successful dictations are not
# printed: the whole point is that a quiet screen means a healthy app.

set -uo pipefail

TRACE="$HOME/Library/Application Support/com.ttp.desktop/ttp-trace.log"

if [[ ! -f "$TRACE" ]]; then
  echo "No trace at $TRACE — is TTP running?" >&2
  exit 1
fi

RED=$'\033[31m'; YEL=$'\033[33m'; DIM=$'\033[2m'; OFF=$'\033[0m'

annotate() {
  # Reads trace lines, prints only the interesting ones with a gloss.
  while IFS= read -r line; do
    case "$line" in
      *stale_fn_cleared*)
        printf '%s%s%s\n    %s↑ THE ONE TO WATCH. The Globe key was latched down and we forced it up.\n      Injected keystrokes would have become system shortcuts.%s\n' \
          "$RED" "$line" "$OFF" "$DIM" "$OFF" ;;
      *paste.modifiers*)
        printf '%s%s%s\n    %s↑ THE ONE TO WATCH. A modifier was still held when we typed.\n      Check the paste.verify line right after this one.%s\n' \
          "$RED" "$line" "$OFF" "$DIM" "$OFF" ;;
      *tap_abandoned*)
        printf '%s%s%s\n    %s↑ The Fn key is dead for this session. Restart TTP.%s\n' \
          "$RED" "$line" "$OFF" "$DIM" "$OFF" ;;
      *dead_capture*)
        printf '%s%s%s\n    %s↑ The microphone delivered pure silence. Check the device name.%s\n' \
          "$RED" "$line" "$OFF" "$DIM" "$OFF" ;;
      *capture.start_failed*|*capture.stop_failed*|*dictation.rejected*)
        printf '%s%s%s\n' "$RED" "$line" "$OFF" ;;
      *'"changed":false'*)
        # Only interesting when Accessibility could actually read the target.
        case "$line" in
          *'"ax_readable":true'*)
            printf '%s%s%s\n    %s↑ Keystrokes were posted and did not land.%s\n' \
              "$RED" "$line" "$OFF" "$DIM" "$OFF" ;;
        esac ;;
      *'"outcome":"aborted"'*)
        printf '%s%s%s\n' "$YEL" "$line" "$OFF" ;;
      *tap_rearmed*|*tap_rebuilt*|*event_dropped*)
        printf '%s%s%s\n' "$YEL" "$line" "$OFF" ;;
      *hotkey.timer_stall*)
        # Only while a dictation is in flight; at rest this is just App Nap
        # doing its job and is expected several hundred times a day.
        : ;;
    esac
  done
}

if [[ "${1:-}" == "--since" ]]; then
  cat "$TRACE" | annotate
  exit 0
fi

echo "Watching $TRACE"
echo "A quiet screen means a healthy app. Ctrl-C to stop."
echo
tail -n 0 -F "$TRACE" | annotate
