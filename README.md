# claude-zellij-monitor

Monitor any number of Claude Code sessions running in zellij tabs, and see at a
glance which ones want you — the tab name gets a 🔴 marker the moment a session
finishes its turn (your move), and a 🟡 marker while it wants permission or has
gone idle. The marker clears automatically when you reply or a tool runs.

No dashboard pane, no polling, no screen-scraping. Detection rides on Claude
Code's own hooks; the marker is painted by a tiny headless zellij plugin.

## How it works

```
Claude Code session (in a zellij pane)
        │  hook fires (Notification / Stop / UserPromptSubmit / SessionEnd)
        ▼
claude-zellij-hook.sh   reads $ZELLIJ_PANE_ID
        │  zellij pipe --plugin claude-monitor.wasm --name claude_status
        ▼
claude-monitor (headless WASM plugin)
        │  maps pane id → tab (PaneManifest), then rename_tab(...)
        ▼
zellij tab bar:  "  build  " → "🔴 build  "
```

- **Notification** → `busy` (🟡 — wants permission, or idle).
- **Stop** → `needs_input` (🔴 — turn finished, your move).
- **PreToolUse** → `working` (a tool is about to run → marker cleared promptly).
- **PostToolUse** → `working` (the tool finished → marker stays cleared).
- **UserPromptSubmit** → `working` (you replied → marker cleared).
- **SessionEnd** → `gone` (Claude no longer running in that pane).

`needs_input` (🔴) outranks `busy` (🟡) when a single tab holds several Claude
panes.

> **Why two colors.** Claude Code emits *no* hook when you **approve** a tool, so
> a pending permission prompt and an approved-but-still-running command look
> identical to the plugin. Rather than show an urgent 🔴 for a session that's
> actually just churning (e.g. watching a long CI job after you approved it),
> permission/idle `Notification`s read as 🟡 *busy*, and only `Stop` — the turn
> genuinely finished — reads as 🔴. The trade-off: a permission prompt you
> haven't approved yet shows 🟡, not 🔴.
>
> `PreToolUse`/`PostToolUse` clear the marker once a tool actually runs (so a
> reply or an approval that leads to tool use clears it); they never fire for an
> *idle* or *turn-finished* state, so those markers persist until you engage.

A tab is only marked while it is **not** the active tab. If Claude finishes while
you're already looking at the tab, it won't nag; focusing a marked tab clears it.

The plugin auto-launches (headless) the first time a hook pipes to it — you don't
need to add it to a layout.

## Install

```sh
./install.sh
```

This builds `claude-monitor.wasm` and copies it plus the hook to
`~/.config/zellij/plugins/`.

Then add this to `~/.claude/settings.json` (merges with your existing config):

```json
{
  "hooks": {
    "Notification": [
      { "matcher": "*", "hooks": [ { "type": "command", "command": "/home/christian/.config/zellij/plugins/claude-zellij-hook.sh busy", "async": true } ] }
    ],
    "Stop": [
      { "matcher": "*", "hooks": [ { "type": "command", "command": "/home/christian/.config/zellij/plugins/claude-zellij-hook.sh needs_input", "async": true } ] }
    ],
    "PreToolUse": [
      { "matcher": "*", "hooks": [ { "type": "command", "command": "/home/christian/.config/zellij/plugins/claude-zellij-hook.sh working", "async": true } ] }
    ],
    "PostToolUse": [
      { "matcher": "*", "hooks": [ { "type": "command", "command": "/home/christian/.config/zellij/plugins/claude-zellij-hook.sh working", "async": true } ] }
    ],
    "UserPromptSubmit": [
      { "matcher": "*", "hooks": [ { "type": "command", "command": "/home/christian/.config/zellij/plugins/claude-zellij-hook.sh working", "async": true } ] }
    ],
    "SessionEnd": [
      { "matcher": "*", "hooks": [ { "type": "command", "command": "/home/christian/.config/zellij/plugins/claude-zellij-hook.sh gone", "async": true } ] }
    ]
  }
}
```

Start a Claude Code session inside a zellij tab. **The first time the plugin
loads, zellij shows a permission prompt** (ReadApplicationState +
ChangeApplicationState) — press `y` to grant it once. After that it's silent.

## Configuration

- **Markers:** `marker` is the 🔴 needs-you (Stop) marker, default `🔴 `;
  `busy_marker` is the 🟡 busy (Notification) marker, default `🟡 `. To change
  either, launch the plugin from a layout with config keys instead of relying on
  auto-launch:

  ```kdl
  // somewhere in your zellij layout
  pane size=1 borderless=true {
      plugin location="file:/home/christian/.config/zellij/plugins/claude-monitor.wasm" {
          marker "🔴 "
          busy_marker "🟡 "
      }
  }
  ```

- **Plugin path override:** set `CLAUDE_MONITOR_PLUGIN` in the hook's environment
  to point at a different `.wasm` URL.

## Notes & limitations

- The plugin marks tabs *per zellij session*; each session auto-launches its own
  headless instance and marks only its own tabs.
- Tab renaming overwrites a manually-set tab name's prefix only — the base name is
  preserved by stripping/re-adding the marker, so your tab names survive.
- Zellij plugins cannot read other panes' text, so detection deliberately comes
  from Claude Code hooks rather than scraping the terminal.

## Develop

```sh
cargo build --release --target wasm32-wasip1 --bin claude-monitor
cp target/wasm32-wasip1/release/claude-monitor.wasm ~/.config/zellij/plugins/claude-monitor.wasm
# hot reload into a running session:
zellij action start-or-reload-plugin file:$HOME/.config/zellij/plugins/claude-monitor.wasm
```

> Note: the plugin is a `[[bin]]`, **not** a `cdylib`. On current Rust
> toolchains, a `cdylib` builds a WASI *command* module (with `_start`) that
> zellij rejects with "could not find exported function". The bin target builds
> the reactor module zellij loads.
