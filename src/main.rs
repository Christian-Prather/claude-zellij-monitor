//! claude-monitor — a headless zellij plugin that marks tabs by their Claude Code
//! session status.
//!
//! Detection happens out-of-band via Claude Code hooks (see claude-zellij-hook.sh):
//! each hook runs `zellij pipe --plugin <this> --name claude_status
//! --args pane=<ZELLIJ_PANE_ID>,status=<needs_input|busy|working|gone>`. This plugin
//! maps the pane id to its tab (via PaneManifest) and renames that tab to add/remove
//! a marker.
//!
//! Two states are shown, because Claude Code emits no event when you *approve* a
//! tool — a pending permission prompt and an approved-but-still-running command look
//! identical to us:
//!   * `busy`  (yellow) — Notification: Claude wants permission or has gone idle. We
//!     can't tell "waiting to be approved" from "approved and now churning", so both
//!     read as busy rather than nagging red.
//!   * `needs_input` (red) — Stop: the turn finished, it's genuinely your move.
//!   * `working` / `gone` clear the marker (you replied, a tool ran, or the session
//!     ended).
//!
//! It deliberately does NOT key off the focused tab: in zellij 0.44 switching tabs
//! emits no plugin event, so any focus-based logic would be unreliable.

use std::collections::{BTreeMap, HashMap};
use zellij_tile::prelude::*;

/// What a pane's Claude session currently wants. Absent from the map = nothing to
/// show. `Needs` (red) outranks `Busy` (yellow) when a tab holds several panes.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PaneStatus {
    Needs,
    Busy,
}

#[derive(Default)]
struct State {
    /// Latest known layout of all panes, keyed by tab position.
    panes: PaneManifest,
    /// Latest known tabs (used for position -> tab_id -> name).
    tabs: Vec<TabInfo>,
    /// Per terminal-pane Claude status. Absent = clear.
    status: HashMap<u32, PaneStatus>,
    /// Marker for a tab that needs you (Stop / turn finished). Configurable via the
    /// plugin `marker` config key; defaults to a red circle + space.
    needs_marker: String,
    /// Marker for a tab that's busy or wants attention (Notification: permission or
    /// idle). Configurable via the plugin `busy_marker` config key; defaults to a
    /// yellow circle + space.
    busy_marker: String,
}

impl State {
    /// Strip whichever status marker (if any) currently prefixes a tab name, leaving
    /// the user's base name untouched.
    fn strip_markers<'a>(&self, name: &'a str) -> &'a str {
        name.strip_prefix(&self.needs_marker)
            .or_else(|| name.strip_prefix(&self.busy_marker))
            .unwrap_or(name)
    }

    /// Highest-priority status among a tab's non-plugin panes: `Needs` (red) outranks
    /// `Busy` (yellow); `None` means clear.
    fn tab_status(&self, position: usize) -> Option<PaneStatus> {
        let panes = self.panes.panes.get(&position)?;
        let mut busy = false;
        for p in panes {
            if p.is_plugin {
                continue;
            }
            match self.status.get(&p.id) {
                Some(PaneStatus::Needs) => return Some(PaneStatus::Needs),
                Some(PaneStatus::Busy) => busy = true,
                None => {}
            }
        }
        busy.then_some(PaneStatus::Busy)
    }

    /// Bring every tab name in line with its panes' Claude status.
    fn reconcile(&self) {
        // Global kill switch: flip to false to strip every marker and add none.
        const ENABLED: bool = true;
        for tab in &self.tabs {
            let base = self.strip_markers(&tab.name).to_string();
            let status = if ENABLED {
                self.tab_status(tab.position)
            } else {
                None
            };
            let desired = match status {
                Some(PaneStatus::Needs) => format!("{}{}", self.needs_marker, base),
                Some(PaneStatus::Busy) => format!("{}{}", self.busy_marker, base),
                None => base,
            };
            if desired != tab.name {
                // Target by STABLE tab_id, not position: the server indexes
                // rename_tab's positional argument differently from
                // TabInfo.position, which lands the marker on the wrong tab and
                // clobbers its name. tab_id is unambiguous.
                rename_tab_with_id(tab.tab_id as u64, &desired);
            }
        }
    }
}

impl ZellijPlugin for State {
    fn load(&mut self, config: BTreeMap<String, String>) {
        self.needs_marker = config
            .get("marker")
            .cloned()
            .unwrap_or_else(|| "🔴 ".to_string());
        self.busy_marker = config
            .get("busy_marker")
            .cloned()
            .unwrap_or_else(|| "🟡 ".to_string());
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);
        subscribe(&[EventType::PaneUpdate, EventType::TabUpdate]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PaneUpdate(manifest) => {
                self.panes = manifest;
                self.reconcile();
                true
            }
            Event::TabUpdate(tabs) => {
                self.tabs = tabs;
                self.reconcile();
                true
            }
            _ => false,
        }
    }

    fn pipe(&mut self, msg: PipeMessage) -> bool {
        if msg.name != "claude_status" {
            return false;
        }
        let Some(pane) = msg.args.get("pane").and_then(|s| s.parse::<u32>().ok()) else {
            return false;
        };
        let status = msg
            .args
            .get("status")
            .map(String::as_str)
            .unwrap_or("needs_input");
        match status {
            "needs_input" => {
                self.status.insert(pane, PaneStatus::Needs);
            }
            "busy" => {
                self.status.insert(pane, PaneStatus::Busy);
            }
            // "working", "gone", anything else -> clear.
            _ => {
                self.status.remove(&pane);
            }
        }
        self.reconcile();
        true
    }

    fn render(&mut self, _rows: usize, _cols: usize) {
        // DEBUG render for validation.
        let t: Vec<String> = self
            .tabs
            .iter()
            .map(|t| format!("{}@{}#{}", t.name, t.position, t.tab_id))
            .collect();
        let p: Vec<String> = self
            .panes
            .panes
            .iter()
            .map(|(pos, v)| {
                let ids: Vec<String> = v
                    .iter()
                    .filter(|x| !x.is_plugin)
                    .map(|x| x.id.to_string())
                    .collect();
                format!("{pos}:[{}]", ids.join(","))
            })
            .collect();
        println!("status={:?}", self.status);
        println!("tabs={t:?}");
        println!("panes={p:?}");
    }
}

register_plugin!(State);
