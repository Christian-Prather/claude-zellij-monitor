#!/usr/bin/env bash
# Build and install the claude-monitor zellij plugin + hook.
set -euo pipefail
cd "$(dirname "$0")"

TARGET=wasm32-wasip1
DEST="$HOME/.config/zellij/plugins"

echo "==> building (release, $TARGET)"
rustup target add "$TARGET" >/dev/null 2>&1 || true
cargo build --release --target "$TARGET" --bin claude-monitor

echo "==> installing to $DEST"
mkdir -p "$DEST"
cp "target/$TARGET/release/claude-monitor.wasm" "$DEST/claude-monitor.wasm"
install -m 0755 claude-zellij-hook.sh "$DEST/claude-zellij-hook.sh"

echo "==> done"
echo "Plugin: $DEST/claude-monitor.wasm"
echo "Hook:   $DEST/claude-zellij-hook.sh"
echo
echo "Next: add the hooks block from README.md to ~/.claude/settings.json,"
echo "then start a Claude Code session inside a zellij tab. The first time the"
echo "plugin loads, approve the zellij permission prompt (y)."
