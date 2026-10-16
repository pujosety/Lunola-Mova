// SPDX-License-Identifier: AGPL-3.0-or-later
//! Timeline preview and edit cursor share one position during hover scrubbing.

/// Update the paused timeline cursor and preview together; report a redraw.
pub(crate) fn scrub(
    playhead: &mut f32,
    hover: &mut Option<f32>,
    seconds: f32,
    end: f32,
    stops_at_end: bool,
) -> bool {
    if !seconds.is_finite() {
        return false;
    }
    let mut at = seconds.max(0.0);
    if stops_at_end {
        at = at.min(end.max(0.0));
    }
    let changed = *playhead != at || *hover != Some(at);
    *playhead = at;
    *hover = Some(at);
    changed
}
