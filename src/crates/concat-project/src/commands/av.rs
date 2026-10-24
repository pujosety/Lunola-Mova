// SPDX-License-Identifier: AGPL-3.0-or-later
//! Lunola AV insertion composes existing placement and detach commands.
use super::*;
use serde_json::{Value, json};

pub(super) fn apply(
    project: &mut Project,
    mint: &mut IdMint,
    command: Command,
) -> Result<Outcome, CommandError> {
    if let Command::SetClipAvLinked { clip_id, linked } = command {
        let Some(group) = project
            .active()
            .clip(&clip_id)
            .and_then(|clip| clip.extra.get("lunolaAvGroup"))
            .cloned()
        else {
            return Ok(Outcome::default());
        };
        let mut changed = false;
        for clip in project
            .active_mut()
            .clips_where(|clip| clip.extra.get("lunolaAvGroup") == Some(&group))
        {
            changed |= clip.extra.get("lunolaAvLinked") != Some(&json!(linked));
            clip.extra.insert("lunolaAvLinked".into(), json!(linked));
        }
        return Ok(Outcome {
            created_id: None,
            applied: changed,
        });
    }
    let Command::AddClipWithAudio {
        media_id,
        track_id,
        start,
        ripple,
        locked_tracks,
    } = command
    else {
        unreachable!("AV command routing")
    };
    let media = project
        .media_by_id(&media_id)
        .ok_or(CommandError::MediaGone)?
        .clone();
    let mut staged = project.clone();
    let track_id = match track_id {
        Some(id) => id,
        None => {
            let existing = staged
                .active()
                .tracks
                .iter()
                .rev()
                .find(|track| {
                    !locked_tracks.contains(&track.id)
                        && track.extra.get("lunolaLaneRole").and_then(Value::as_str)
                            != Some("audio")
                })
                .map(|track| track.id.clone());
            match existing {
                Some(id) => id,
                None => {
                    super::tracks::apply(&mut staged, mint, Command::AddTrack { bottom: false })?
                        .created_id
                        .expect("new visual lane")
                }
            }
        }
    };
    if locked_tracks.contains(&track_id) {
        return Ok(Outcome::default());
    }
    let outcome = super::clips::apply(
        &mut staged,
        mint,
        Command::AddClip {
            media_id,
            track_id,
            start,
            ripple,
        },
    )?;
    let Some(video_id) = outcome.created_id.clone() else {
        return Ok(outcome);
    };
    // The existing magnetic placement moves video on its own lane. Move
    // only explicitly linked sound by the same measured delta; unrelated
    // music and legacy detached audio keep their original positions.
    let moves: Vec<ClipMove> = project
        .active()
        .clips
        .iter()
        .filter_map(|sound| {
            if sound.kind != ClipKind::Audio
                || sound.extra.get("lunolaAvLinked") != Some(&json!(true))
            {
                return None;
            }
            let group = sound.extra.get("lunolaAvGroup")?;
            let old_video = project.active().clips.iter().find(|clip| {
                clip.kind == ClipKind::Video
                    && clip.extra.get("lunolaAvGroup") == Some(group)
                    && (clip.start - sound.start).abs() < 1e-6
                    && (clip.duration - sound.duration).abs() < 1e-6
            })?;
            let after = staged.active().clip(&old_video.id)?;
            let delta = after.start - old_video.start;
            (delta.abs() > 1e-6).then(|| ClipMove {
                clip_id: sound.id.clone(),
                start: sound.start + delta,
                track_id: sound.track_id.clone(),
            })
        })
        .collect();
    if moves
        .iter()
        .any(|wanted| locked_tracks.contains(&wanted.track_id))
    {
        return Ok(Outcome::default());
    }
    if !moves.is_empty() {
        // Refuse instead of allowing collision resolution to desynchronise AV.
        if staged.active().resolve_moves(&moves).as_ref() != Some(&moves) {
            return Ok(Outcome::default());
        }
        super::clips::apply(&mut staged, mint, Command::MoveClips { moves })?;
    }
    if media.kind == MediaKind::Video && media.has_audio {
        super::audio::apply(
            &mut staged,
            mint,
            Command::DetachAudio {
                clip_id: video_id.clone(),
            },
        )?;
        let sounds: Vec<String> = staged
            .active()
            .clips
            .iter()
            .filter(|clip| clip.detached_from.as_deref() == Some(&video_id))
            .map(|clip| clip.id.clone())
            .collect();
        for id in &sounds {
            let sound = staged.active().clip(id).expect("detached sound").clone();
            let video = staged.active().clip(&video_id).expect("new video");
            let video_index = staged
                .active()
                .tracks
                .iter()
                .position(|track| track.id == video.track_id)
                .expect("video lane");
            let candidate = staged
                .active()
                .tracks
                .iter()
                .take(video_index)
                .rev()
                .find(|track| {
                    track.visible
                        && !track.muted
                        && !locked_tracks.contains(&track.id)
                        && track.extra.get("lunolaLaneRole").and_then(Value::as_str)
                            == Some("audio")
                        && !staged.active().clips.iter().any(|clip| {
                            clip.id != sound.id
                                && clip.track_id == track.id
                                && clip.start < sound.start + sound.duration
                                && sound.start < clip.start + clip.duration
                        })
                })
                .map(|track| track.id.clone());
            let audio_track = candidate.unwrap_or_else(|| {
                let id = mint.next("t");
                let mut track = Track {
                    id: id.clone(),
                    visible: true,
                    muted: false,
                    extra: Default::default(),
                };
                track.extra.insert("lunolaLaneRole".into(), json!("audio"));
                staged.active_mut().tracks.insert(video_index, track);
                id
            });
            super::clips::apply(
                &mut staged,
                mint,
                Command::MoveClips {
                    moves: vec![ClipMove {
                        clip_id: id.clone(),
                        start: sound.start,
                        track_id: audio_track,
                    }],
                },
            )?;
        }
        let mut group = sounds;
        group.push(video_id.clone());
        for id in group {
            let clip = staged.active_mut().clip_mut(&id).expect("AV group");
            clip.extra.insert("lunolaAvGroup".into(), json!(video_id));
            clip.extra.insert("lunolaAvLinked".into(), json!(true));
        }
    }
    *project = staged;
    Ok(outcome)
}
