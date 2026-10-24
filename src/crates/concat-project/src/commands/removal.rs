// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors
//! Atomic, non-destructive background-removal selection.
use super::*;

/// Inspector removal selection, not an extension of the Cutout schema.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RemovalMode {
    /// Bypass background removal.
    Off,
    /// Automatic source-space mask.
    Auto,
    /// Source-space mask with retained corrections.
    Custom,
    /// Colour key in the existing picture chain.
    Chroma,
}

const RETAINED_CUTOUT: &str = "lunolaRemovalRetainedCutoutV1";

fn is_chroma(id: &str) -> bool {
    matches!(
        id,
        "concat.chroma-key" | "concat.green-screen" | "concat.blue-screen"
    )
}

fn retained_cutout(clip: &Clip) -> Result<Option<Cutout>, CommandError> {
    let Some(value) = clip.extra.get(RETAINED_CUTOUT) else {
        return Ok(None);
    };
    // Refuse unsupported/colliding payloads instead of silently dropping
    // future fields, or replacing an independently active mask.
    let valid_keys = |value: &serde_json::Value, known: &[&str]| {
        value
            .as_object()
            .is_some_and(|object| object.keys().all(|key| known.contains(&key.as_str())))
    };
    if clip.cutout.is_some()
        || !valid_keys(
            value,
            &[
                "mode",
                "subject",
                "feather",
                "expansion",
                "edge_strength",
                "edge_radius",
                "background_mode",
                "background_blur",
                "background_color",
                "strokes",
            ],
        )
    {
        return Err(CommandError::InvalidRemovalMetadata);
    }
    if let Some(strokes) = value.get("strokes").and_then(serde_json::Value::as_array) {
        if strokes
            .iter()
            .any(|stroke| !valid_keys(stroke, &["tool", "size", "points", "at"]))
        {
            return Err(CommandError::InvalidRemovalMetadata);
        }
    }
    let cutout: Cutout =
        serde_json::from_value(value.clone()).map_err(|_| CommandError::InvalidRemovalMetadata)?;
    if cutout.clone().tidy() != cutout {
        return Err(CommandError::InvalidRemovalMetadata);
    }
    Ok(Some(cutout))
}

pub(super) fn apply(project: &mut Project, command: Command) -> Result<Outcome, CommandError> {
    let Command::SetClipRemovalMode { clip_id, mode } = command else {
        unreachable!("removal routing")
    };
    let Some(original) = project
        .active()
        .clips
        .iter()
        .find(|clip| clip.id == clip_id)
    else {
        return Ok(Outcome::default());
    };
    // File-backed pictures only; never turn audio/titles/shapes/layers into mask jobs.
    let visual = project.media.iter().any(|media| {
        media.id == original.media_id
            && matches!(
                (original.kind, media.kind),
                (ClipKind::Video, MediaKind::Video) | (ClipKind::Image, MediaKind::Image)
            )
    });
    if !visual {
        return Ok(Outcome::default());
    }
    let retained = retained_cutout(original)?;
    // Stage all changes, including metadata, before publishing a single clip.
    let mut next = (**original).clone();
    if matches!(mode, RemovalMode::Off | RemovalMode::Chroma) {
        if let Some(cutout) = next.cutout.take() {
            next.extra.insert(
                RETAINED_CUTOUT.into(),
                serde_json::to_value(cutout).map_err(|_| CommandError::InvalidRemovalMetadata)?,
            );
        }
    } else {
        let mut cutout = next.cutout.take().or(retained).unwrap_or_else(Cutout::auto);
        cutout.mode = if mode == RemovalMode::Auto {
            CutoutMode::Auto
        } else {
            CutoutMode::Custom
        };
        next.extra.remove(RETAINED_CUTOUT);
        next.cutout = Some(cutout);
    }
    if mode != RemovalMode::Chroma {
        for effect in &mut next.video_effects {
            if is_chroma(&effect.id) {
                effect.enabled = false;
            }
        }
    } else {
        // Keep the currently active keyer if present; otherwise re-enable
        // the first retained keyer anywhere in the complete chain.
        let selected = next
            .video_effects
            .iter()
            .position(|effect| is_chroma(&effect.id) && effect.enabled)
            .or_else(|| {
                next.video_effects
                    .iter()
                    .position(|effect| is_chroma(&effect.id))
            });
        if let Some(selected) = selected {
            for (index, effect) in next.video_effects.iter_mut().enumerate() {
                if is_chroma(&effect.id) {
                    effect.enabled = index == selected;
                }
            }
        } else {
            next.video_effects
                .push(AppliedFilter::new("concat.chroma-key"));
        }
    }
    let applied = next != **original;
    if applied {
        *project.active_mut().clip_mut(&clip_id).unwrap() = next;
    }
    Ok(Outcome {
        applied,
        created_id: None,
    })
}
