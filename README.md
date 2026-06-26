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

The marker reflects hook state, not which tab you're looking at: zellij 0.44 emits
no plugin event when you switch tabs, so focus can't be tracked reliably. A marker
clears when the session reports `working` (you replied, or a tool ran after you
approved it) or `gone` (the session ended) — not merely by viewing the tab.

The plugin auto-launches (headless) the first time a hook pipes to it — you don't
need to add it to a layout.

## Install

**Prerequisites:** zellij 0.44.x (the plugin ABI is pinned to it) and Claude Code.

### 1. Download & place the plugin

zellij loads a plugin from a `.wasm` file referenced by a `file:`/`https:`
location; the convention is to keep local plugins in `~/.config/zellij/plugins/`.
Each tagged release attaches the prebuilt wasm and the hook script, so grab both:

```sh
mkdir -p ~/.config/zellij/plugins
base="https://github.com/Christian-Prather/claude-zellij-monitor/releases/latest/download"
curl -fsSL "$base/claude-monitor.wasm"   -o ~/.config/zellij/plugins/claude-monitor.wasm
curl -fsSL "$base/claude-zellij-hook.sh" -o ~/.config/zellij/plugins/claude-zellij-hook.sh
chmod +x ~/.config/zellij/plugins/claude-zellij-hook.sh
```

You don't register the plugin in a layout — it auto-launches headless the first
time a hook pipes to it.

<details>
<summary>Prefer to build from source? (needs a Rust toolchain)</summary>

```sh
./install.sh
```

This runs `cargo build --release --target wasm32-wasip1` and copies
`claude-monitor.wasm` + `claude-zellij-hook.sh` to `~/.config/zellij/plugins/`
(adding the `wasm32-wasip1` target if needed) — same destination as the download
above.

</details>

### 2. Wire up the Claude Code hooks

Add this to `~/.claude/settings.json` (it merges with your existing config). The
`$HOME` in each command is expanded by the shell that runs the hook:

```json
{
  "hooks": {
    "Notification": [
      { "matcher": "*", "hooks": [ { "type": "command", "command": "$HOME/.config/zellij/plugins/claude-zellij-hook.sh busy", "async": true } ] }
    ],
    "Stop": [
      { "matcher": "*", "hooks": [ { "type": "command", "command": "$HOME/.config/zellij/plugins/claude-zellij-hook.sh needs_input", "async": true } ] }
    ],
    "PreToolUse": [
      { "matcher": "*", "hooks": [ { "type": "command", "command": "$HOME/.config/zellij/plugins/claude-zellij-hook.sh working", "async": true } ] }
    ],
    "PostToolUse": [
      { "matcher": "*", "hooks": [ { "type": "command", "command": "$HOME/.config/zellij/plugins/claude-zellij-hook.sh working", "async": true } ] }
    ],
    "UserPromptSubmit": [
      { "matcher": "*", "hooks": [ { "type": "command", "command": "$HOME/.config/zellij/plugins/claude-zellij-hook.sh working", "async": true } ] }
    ],
    "SessionEnd": [
      { "matcher": "*", "hooks": [ { "type": "command", "command": "$HOME/.config/zellij/plugins/claude-zellij-hook.sh gone", "async": true } ] }
    ]
  }
}
```

### 3. First run

Start a Claude Code session inside a zellij tab. **The first time the plugin
loads, zellij shows a permission prompt** (ReadApplicationState +
ChangeApplicationState) — press `y` to grant it once. After that it's silent.
Hooks are read at session start, so restart any sessions that were already running
when you edited `settings.json`.

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

### Layout & tests

The workspace splits into two crates:

- the root `claude-monitor` bin — the wasm plugin; wires zellij's
  `PaneManifest`/`TabInfo` into the marker logic and calls the host.
- [`core/`](core/) (`claude-monitor-core`) — the pure marker logic (status
  precedence, marker strip/apply), deliberately free of `zellij-tile` so it
  builds and unit-tests on the host. (`zellij-tile` drags in host-only system
  libs that don't compile off-wasm, which is why the logic is split out.)

```sh
cargo test -p claude-monitor-core   # unit tests, run on the host
cargo fmt --all -- --check          # formatting
cargo clippy -p claude-monitor-core -- -D warnings
```

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs those checks,
builds the wasm plugin (uploading `claude-monitor.wasm` as an artifact), and
shellchecks the scripts on every push and PR. Pushing a `v*` tag triggers
[`release.yml`](.github/workflows/release.yml), which builds the plugin and
attaches `claude-monitor.wasm` + the hook to a GitHub Release.
