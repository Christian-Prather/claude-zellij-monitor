#!/usr/bin/env bash
# claude-zellij-hook.sh <status>
#
# Called by Claude Code hooks. Reports the current zellij pane's Claude status
# to the claude-monitor plugin via `zellij pipe`. The plugin maps the pane id to
# its tab and marks the tab name when the session needs your attention.
#
#   status = needs_input | working
#
# Wired up in ~/.claude/settings.json:
#   Notification      -> needs_input   (permission prompt / idle)
#   Stop              -> needs_input   (finished, waiting for you)
#   UserPromptSubmit  -> working       (you replied; clear the marker)
#   SessionEnd        -> gone          (pane no longer running Claude)

status="${1:-needs_input}"

# Drain the hook JSON on stdin so Claude never blocks writing to us.
cat >/dev/null 2>&1 || true

# Only meaningful inside a zellij session with a known pane.
[ -n "${ZELLIJ:-}" ] || exit 0
[ -n "${ZELLIJ_PANE_ID:-}" ] || exit 0

PLUGIN="${CLAUDE_MONITOR_PLUGIN:-file:$HOME/.config/zellij/plugins/claude-monitor.wasm}"

# `zellij pipe --plugin` auto-launches the plugin (headless) if it isn't running.
zellij pipe \
  --plugin "$PLUGIN" \
  --name claude_status \
  --args "pane=${ZELLIJ_PANE_ID},status=${status}" \
  >/dev/null 2>&1 || true

exit 0
