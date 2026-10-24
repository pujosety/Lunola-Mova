// SPDX-License-Identifier: AGPL-3.0-or-later
//! Explicit AV timing groups; legacy detach provenance is not a link.
use concat_project::commands::{Command, TrimEdge};
use concat_project::model::{ClipKind, Timeline};

/// Stable display roles without changing compositing order or track IDs.
pub(crate) fn lane_label(timeline: &Timeline, id: &str) -> (String, bool) {
    let role = |id: &str| {
        let kinds: HashSet<_> = timeline
            .clips
            .iter()
            .filter(|clip| clip.track_id == id)
            .map(|clip| match clip.kind {
                ClipKind::Audio => "A",
                ClipKind::Video | ClipKind::Image => "V",
                ClipKind::Text => "T",
                _ => "FX",
            })
            .collect();
        if kinds.len() > 1 {
            "Mix"
        } else if let Some(kind) = kinds.iter().next() {
            *kind
        } else if timeline
            .track(id)
            .and_then(|track| track.extra.get("lunolaLaneRole"))
            .and_then(Value::as_str)
            == Some("audio")
        {
            "A"
        } else {
            ""
        }
    };
    let kind = role(id);
    if kind.is_empty() {
        return (String::new(), false);
    }
    let mut number = 0;
    for track in timeline.tracks.iter().rev() {
        if role(&track.id) == kind {
            number += 1;
        }
        if track.id == id {
            break;
        }
    }
    (format!("{kind}{number}"), kind == "A")
}

/// Plan a common edge delta for explicitly linked AV clips.
pub(crate) fn trim_plan(
    timeline: &Timeline,
    id: &str,
    edge: TrimEdge,
    delta: f64,
    ripple: bool,
    locked: impl Fn(&str) -> bool,
) -> Option<Command> {
    if !delta.is_finite() {
        return None;
    }
    let ids = targets(timeline, &[id.to_owned()], locked)?;
    let mut floor = f64::NEG_INFINITY;
    let mut ceiling = f64::INFINITY;
    for id in &ids {
        let clip = timeline.clip(id)?;
        let (before, after) = if ripple {
            (0.0, f64::INFINITY)
        } else {
            timeline.room_around(id)?
        };
        match edge {
            TrimEdge::Start => {
                floor = floor.max(-clip.source_start / clip.speed.max(0.01));
                if !ripple {
                    floor = floor.max(before - clip.start);
                }
                ceiling = ceiling.min(clip.duration - 1.0 / 60.0);
            }
            TrimEdge::End => {
                floor = floor.max(1.0 / 60.0 - clip.duration);
                ceiling = ceiling.min(after - clip.start - clip.duration);
            }
        }
    }
    if floor > ceiling {
        return None;
    }
    let delta = delta.clamp(floor, ceiling);
    if delta.abs() < 1e-6 {
        return None;
    }
    Some(Command::Batch {
        commands: ids
            .into_iter()
            .map(|clip_id| Command::TrimClip {
                clip_id,
                edge,
                delta,
                ripple,
            })
            .collect(),
    })
}
use serde_json::Value;
use std::collections::HashSet;

/// Expand timing edits to aligned linked partners; refuse locked partners.
pub(crate) fn targets(
    timeline: &Timeline,
    ids: &[String],
    locked: impl Fn(&str) -> bool,
) -> Option<Vec<String>> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for id in ids {
        let clip = timeline.clip(id)?;
        if locked(&clip.track_id) {
            return None;
        }
        if seen.insert(id.clone()) {
            result.push(id.clone());
        }
        if clip.extra.get("lunolaAvLinked").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        let Some(group) = clip.extra.get("lunolaAvGroup") else {
            continue;
        };
        for partner in &timeline.clips {
            if partner.extra.get("lunolaAvGroup") != Some(group)
                || (partner.start - clip.start).abs() > 1e-6
                || (partner.duration - clip.duration).abs() > 1e-6
            {
                continue;
            }
            if locked(&partner.track_id)
                || partner.extra.get("lunolaAvLinked").and_then(Value::as_bool) != Some(true)
                || (partner.source_start - clip.source_start).abs() > 1e-6
                || (partner.speed - clip.speed).abs() > 1e-6
            {
                return None;
            }
            if seen.insert(partner.id.clone()) {
                result.push(partner.id.clone());
            }
        }
    }
    Some(result)
}
