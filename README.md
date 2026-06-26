# claude-zellij-monitor

Monitor any number of Claude Code sessions running in zellij tabs, and see at a
glance which ones need your input — the tab name gets a 🔴 marker the moment a
session asks for a permission, goes idle, or finishes a turn. The marker clears
automatically when you switch to that tab or reply.

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

- **Notification** → `needs_input` (permission prompt or idle).
- **Stop** → `needs_input` (turn finished, waiting on you).
- **UserPromptSubmit** → `working` (you replied → marker cleared).
- **SessionEnd** → `gone` (Claude no longer running in that pane).

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
      { "matcher": "*", "hooks": [ { "type": "command", "command": "/home/christian/.config/zellij/plugins/claude-zellij-hook.sh needs_input", "async": true } ] }
    ],
    "Stop": [
      { "matcher": "*", "hooks": [ { "type": "command", "command": "/home/christian/.config/zellij/plugins/claude-zellij-hook.sh needs_input", "async": true } ] }
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

- **Marker:** defaults to `🔴 `. To change it, launch the plugin from a layout
  with a config key instead of relying on auto-launch:

  ```kdl
  // somewhere in your zellij layout
  pane size=1 borderless=true {
      plugin location="file:/home/christian/.config/zellij/plugins/claude-monitor.wasm" {
          marker "● "
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
