//! claude-monitor — a headless zellij plugin that marks tabs whose Claude Code
//! session needs your input.
//!
//! Detection happens out-of-band via Claude Code hooks (see claude-zellij-hook.sh):
//! each hook runs `zellij pipe --plugin <this> --name claude_status
//! --args pane=<ZELLIJ_PANE_ID>,status=<needs_input|working>`. This plugin maps the
//! pane id to its tab (via PaneManifest) and renames that tab to add/remove a marker.
//!
//! Model: a tab is marked while any Claude pane in it needs you (its session fired
//! Stop / a permission / idle Notification), and unmarked when that session reports
//! `working` (you replied) or `gone` (session ended). It deliberately does NOT key
//! off the focused tab: in zellij 0.44 switching tabs emits no plugin event, so any
//! focus-based logic would be unreliable.

use std::collections::{BTreeMap, HashSet};
use zellij_tile::prelude::*;

#[derive(Default)]
struct State {
    /// Latest known layout of all panes, keyed by tab position.
    panes: PaneManifest,
    /// Latest known tabs (used for position -> tab_id -> name).
    tabs: Vec<TabInfo>,
    /// Terminal pane ids whose Claude session currently needs the user.
    needs: HashSet<u32>,
    /// Marker prepended to a tab name when it needs input. Configurable via the
    /// plugin `marker` config key; defaults to a red circle + space.
    marker: String,
}

impl State {
    /// Bring every tab name in line with whether any of its panes needs input.
    fn reconcile(&self) {
        // Global kill switch: flip to false to strip every marker and add none.
        const ENABLED: bool = true;
        for tab in &self.tabs {
            let base = tab
                .name
                .strip_prefix(&self.marker)
                .unwrap_or(&tab.name)
                .to_string();
            let needy = ENABLED
                && self
                    .panes
                    .panes
                    .get(&tab.position)
                    .map(|panes| {
                        panes
                            .iter()
                            .any(|p| !p.is_plugin && self.needs.contains(&p.id))
                    })
                    .unwrap_or(false);
            let desired = if needy {
                format!("{}{}", self.marker, base)
            } else {
                base
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
        self.marker = config
            .get("marker")
            .cloned()
            .unwrap_or_else(|| "🔴 ".to_string());
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
        let status = msg.args.get("status").map(String::as_str).unwrap_or("needs_input");
        match status {
            "needs_input" => self.needs.insert(pane),
            // "working", "gone", anything else -> no longer needs input.
            _ => self.needs.remove(&pane),
        };
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
        println!("needs={:?}", self.needs);
        println!("tabs={t:?}");
        println!("panes={p:?}");
    }
}

register_plugin!(State);
