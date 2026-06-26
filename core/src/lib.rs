//! Pure marker logic for claude-monitor, kept free of any `zellij-tile` dependency
//! so it builds and unit-tests on the host target. The wasm plugin crate
//! (`../src/main.rs`) wires zellij's `PaneManifest`/`TabInfo` into these functions.

/// What a pane's Claude session currently wants. `Needs` (red) outranks `Busy`
/// (yellow) when a single tab holds several panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneStatus {
    /// The turn finished — it's your move (red).
    Needs,
    /// Claude wants permission or has gone idle (yellow).
    Busy,
}

impl PaneStatus {
    /// Map a hook `status=` value to a pane status. `needs_input` → `Needs`,
    /// `busy` → `Busy`; everything else (`working`, `gone`, unknown) clears → `None`.
    pub fn from_hook(status: &str) -> Option<PaneStatus> {
        match status {
            "needs_input" => Some(PaneStatus::Needs),
            "busy" => Some(PaneStatus::Busy),
            _ => None,
        }
    }
}

/// Highest-priority status among a tab's panes: `Needs` outranks `Busy`; `None`
/// means the tab should carry no marker.
pub fn tab_status<I>(pane_statuses: I) -> Option<PaneStatus>
where
    I: IntoIterator<Item = Option<PaneStatus>>,
{
    let mut busy = false;
    for s in pane_statuses {
        match s {
            Some(PaneStatus::Needs) => return Some(PaneStatus::Needs),
            Some(PaneStatus::Busy) => busy = true,
            None => {}
        }
    }
    busy.then_some(PaneStatus::Busy)
}

/// Strip whichever status marker (if any) currently prefixes `name`, leaving the
/// user's base name. Tries the needs marker first, then the busy marker.
pub fn strip_markers<'a>(name: &'a str, needs_marker: &str, busy_marker: &str) -> &'a str {
    name.strip_prefix(needs_marker)
        .or_else(|| name.strip_prefix(busy_marker))
        .unwrap_or(name)
}

/// The tab name a given `status` should produce from the current `name`. Idempotent:
/// it strips any existing marker first, so re-applying never stacks markers and the
/// base name always survives.
pub fn desired_name(
    name: &str,
    status: Option<PaneStatus>,
    needs_marker: &str,
    busy_marker: &str,
) -> String {
    let base = strip_markers(name, needs_marker, busy_marker);
    match status {
        Some(PaneStatus::Needs) => format!("{needs_marker}{base}"),
        Some(PaneStatus::Busy) => format!("{busy_marker}{base}"),
        None => base.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: &str = "🔴 ";
    const YEL: &str = "🟡 ";

    #[test]
    fn hook_status_mapping() {
        assert_eq!(
            PaneStatus::from_hook("needs_input"),
            Some(PaneStatus::Needs)
        );
        assert_eq!(PaneStatus::from_hook("busy"), Some(PaneStatus::Busy));
        assert_eq!(PaneStatus::from_hook("working"), None);
        assert_eq!(PaneStatus::from_hook("gone"), None);
        assert_eq!(PaneStatus::from_hook("anything-else"), None);
    }

    #[test]
    fn tab_status_empty_is_clear() {
        assert_eq!(tab_status(std::iter::empty()), None);
        assert_eq!(tab_status([None, None]), None);
    }

    #[test]
    fn tab_status_busy_when_only_busy() {
        assert_eq!(
            tab_status([None, Some(PaneStatus::Busy)]),
            Some(PaneStatus::Busy)
        );
    }

    #[test]
    fn tab_status_needs_outranks_busy() {
        // Order must not matter: Needs wins regardless of position.
        assert_eq!(
            tab_status([Some(PaneStatus::Busy), Some(PaneStatus::Needs)]),
            Some(PaneStatus::Needs)
        );
        assert_eq!(
            tab_status([Some(PaneStatus::Needs), Some(PaneStatus::Busy)]),
            Some(PaneStatus::Needs)
        );
    }

    #[test]
    fn desired_name_adds_marker() {
        assert_eq!(
            desired_name("build", Some(PaneStatus::Needs), RED, YEL),
            "🔴 build"
        );
        assert_eq!(
            desired_name("build", Some(PaneStatus::Busy), RED, YEL),
            "🟡 build"
        );
        assert_eq!(desired_name("build", None, RED, YEL), "build");
    }

    #[test]
    fn desired_name_is_idempotent_and_swaps_color() {
        // Re-applying the same status never stacks markers.
        let red = desired_name("build", Some(PaneStatus::Needs), RED, YEL);
        assert_eq!(
            desired_name(&red, Some(PaneStatus::Needs), RED, YEL),
            "🔴 build"
        );
        // Switching color strips the old marker before adding the new one.
        assert_eq!(
            desired_name(&red, Some(PaneStatus::Busy), RED, YEL),
            "🟡 build"
        );
        // Clearing restores the bare base name.
        assert_eq!(desired_name(&red, None, RED, YEL), "build");
    }

    #[test]
    fn base_name_with_marker_like_characters_survives() {
        // A base name that merely contains an emoji (not as the marker prefix) is
        // preserved.
        let name = "deploy 🔴 prod";
        assert_eq!(strip_markers(name, RED, YEL), "deploy 🔴 prod");
        assert_eq!(
            desired_name(name, Some(PaneStatus::Busy), RED, YEL),
            "🟡 deploy 🔴 prod"
        );
    }

    #[test]
    fn custom_markers_are_respected() {
        assert_eq!(
            desired_name("x", Some(PaneStatus::Needs), "● ", "○ "),
            "● x"
        );
        assert_eq!(strip_markers("○ x", "● ", "○ "), "x");
    }
}
