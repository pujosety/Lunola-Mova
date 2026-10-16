// SPDX-License-Identifier: AGPL-3.0-or-later
//! Lunola's Q/W editing adapter: use existing undoable engine commands.

use concat_project::{
    commands::{Command, TrimEdge},
    model::Timeline,
};

/// Plan a lane-local ripple trim and its new playhead position.
/// Explicit clip selection targets lanes; otherwise the topmost unlocked
/// clip under the playhead is the target. Locked lanes are never edited.
pub(crate) fn plan(
    timeline: &Timeline,
    selection: &[String],
    at: f64,
    edge: TrimEdge,
    locked: impl Fn(&str) -> bool,
) -> Option<(Command, f64)> {
    if !at.is_finite() {
        return None;
    }
    let minimum = 1.0 / 60.0;
    let eligible = |clip: &&std::sync::Arc<concat_project::model::Clip>| {
        !locked(&clip.track_id)
            && at > clip.start + minimum
            && at < clip.start + clip.duration - minimum
    };
    let clips: Vec<_> = if selection.is_empty() {
        vec![timeline.tracks.iter().rev().find_map(|track| {
            timeline
                .clips
                .iter()
                .filter(eligible)
                .find(|clip| clip.track_id == track.id)
        })?]
    } else {
        timeline
            .clips
            .iter()
            .filter(eligible)
            .filter(|clip| selection.contains(&clip.id))
            .collect()
    };
    let ids: Vec<String> = clips.iter().map(|clip| clip.id.clone()).collect();
    let ids = crate::timeline_av::targets(timeline, &ids, &locked)?;
    let clips: Vec<_> = ids.iter().filter_map(|id| timeline.clip(id)).collect();
    let first = clips.first()?;
    let boundary = |clip: &concat_project::model::Clip| match edge {
        TrimEdge::Start => clip.start,
        TrimEdge::End => clip.start + clip.duration,
    };
    // A lane-local command must not silently desynchronise a selected AV
    // group. Different edit boundaries require explicit track targeting,
    // which this adapter does not pretend to provide.
    let mut tracks = std::collections::HashSet::new();
    if clips.iter().any(|clip| {
        !tracks.insert(&clip.track_id) || (boundary(clip) - boundary(first)).abs() > 1e-6
    }) {
        return None;
    }
    let delta = at - boundary(first);
    let playhead = if edge == TrimEdge::Start {
        first.start
    } else {
        at
    };
    let commands = clips
        .iter()
        .map(|clip| Command::TrimClip {
            clip_id: clip.id.clone(),
            edge,
            delta,
            ripple: true,
        })
        .collect();
    Some((Command::Batch { commands }, playhead))
}

#[cfg(test)]
mod tests {
    use super::*;
    use concat_project::{
        commands::{IdMint, apply},
        model::{Clip, ClipKind, Project},
    };
    use std::sync::Arc;

    fn project() -> Project {
        let mut project = Project::new();
        let mut current = Clip::blank("current", "T1", ClipKind::Video, "current", 2.0, 8.0);
        current.source_start = 3.0;
        current.speed = 2.0;
        project.active_mut().clips = vec![
            Arc::new(current),
            Arc::new(Clip::blank(
                "next",
                "T1",
                ClipKind::Video,
                "next",
                10.0,
                4.0,
            )),
            Arc::new(Clip::blank(
                "other",
                "T2",
                ClipKind::Audio,
                "other",
                10.0,
                4.0,
            )),
        ];
        project
    }

    #[test]
    fn locked_selection_does_not_fall_back_to_an_unselected_lane() {
        let project = project();
        for edge in [TrimEdge::Start, TrimEdge::End] {
            assert!(
                plan(
                    project.active(),
                    &["current".into()],
                    5.0,
                    edge,
                    |track| track == "T1"
                )
                .is_none()
            );
        }
    }

    #[test]
    fn gaps_clip_edges_and_non_finite_playheads_are_no_ops() {
        let project = project();
        for edge in [TrimEdge::Start, TrimEdge::End] {
            for at in [0.0, 2.0, 10.0, 15.0, f64::NAN, f64::INFINITY] {
                assert!(plan(project.active(), &["current".into()], at, edge, |_| false).is_none());
            }
        }
    }

    #[test]
    fn incompatible_selected_boundaries_are_not_silently_desynchronised() {
        let mut project = project();
        project.active_mut().clips.push(Arc::new(Clip::blank(
            "sound",
            "T2",
            ClipKind::Audio,
            "sound",
            3.0,
            8.0,
        )));
        for edge in [TrimEdge::Start, TrimEdge::End] {
            assert!(
                plan(
                    project.active(),
                    &["current".into(), "sound".into()],
                    5.0,
                    edge,
                    |_| false
                )
                .is_none()
            );
        }
    }

    #[test]
    fn aligned_selected_video_and_audio_trim_together_in_one_undo_step() {
        use concat_project::{
            doc::{DocumentSettings, to_document},
            editor::Editor,
        };
        let mut project = project();
        project.active_mut().clips.push(Arc::new(Clip::blank(
            "sound",
            "T2",
            ClipKind::Audio,
            "sound",
            2.0,
            8.0,
        )));
        project.media.push(concat_project::model::MediaItem {
            id: "qa".into(),
            path: "qa.mp4".into(),
            duration: Some(60.0),
            ..Default::default()
        });
        for clip in &mut project.active_mut().clips {
            Arc::make_mut(clip).media_id = "qa".into();
        }
        let mut editor = Editor::from_document(&to_document(
            &DocumentSettings {
                name: "QW QA".into(),
                width: 1920,
                height: 1080,
                rate_num: 30,
                rate_den: 1,
            },
            &project,
        ))
        .unwrap();
        let before = editor.project().clone();
        let (command, _) = plan(
            editor.project().active(),
            &["current".into(), "sound".into()],
            5.0,
            TrimEdge::Start,
            |_| false,
        )
        .unwrap();
        editor.apply(command).unwrap();
        assert_eq!(editor.project().active().clips[0].duration, 5.0);
        assert_eq!(editor.project().active().clips[3].duration, 5.0);
        assert_eq!(editor.project().active().clips[2].start, 7.0);
        let after = editor.project().clone();
        assert!(editor.undo());
        assert_eq!(editor.project(), &before);
        assert!(!editor.undo(), "one shortcut is one undo step");
        assert!(editor.redo());
        assert_eq!(editor.project(), &after);
        let reopened = Editor::from_document(&editor.to_document(&DocumentSettings {
            name: "QW QA".into(),
            width: 1920,
            height: 1080,
            rate_num: 30,
            rate_den: 1,
        }))
        .unwrap();
        assert_eq!(reopened.project(), &after);
    }

    #[test]
    fn no_selection_targets_the_topmost_unlocked_clip_at_the_playhead() {
        let mut project = project();
        project.active_mut().clips.push(Arc::new(Clip::blank(
            "top",
            "T3",
            ClipKind::Video,
            "top",
            2.0,
            8.0,
        )));
        let (command, _) = plan(project.active(), &[], 5.0, TrimEdge::Start, |track| {
            track == "T3"
        })
        .expect("Q should work without a selection and skip locked lanes");
        apply(&mut project, &mut IdMint::default(), command).unwrap();
        assert_eq!(project.active().clips[0].duration, 5.0);
        assert_eq!(project.active().clips[3].duration, 8.0);
    }

    #[test]
    fn w_removes_the_tail_and_closes_the_lane_using_the_existing_engine() {
        let mut project = project();
        let (command, playhead) = plan(
            project.active(),
            &["current".into()],
            5.0,
            TrimEdge::End,
            |_| false,
        )
        .expect("W should trim the selected clip at the playhead");
        apply(&mut project, &mut IdMint::default(), command).unwrap();
        let current = &project.active().clips[0];
        assert_eq!(current.start, 2.0);
        assert_eq!(current.duration, 3.0);
        assert_eq!(current.source_start, 3.0);
        assert_eq!(project.active().clips[1].start, 5.0);
        assert_eq!(project.active().clips[2].start, 10.0);
        assert_eq!(playhead, 5.0);
    }

    #[test]
    fn q_removes_the_head_and_closes_the_lane_using_the_existing_engine() {
        let mut project = project();
        let (command, playhead) = plan(
            project.active(),
            &["current".into()],
            5.0,
            TrimEdge::Start,
            |_| false,
        )
        .expect("Q should trim the selected clip at the playhead");
        apply(&mut project, &mut IdMint::default(), command).unwrap();
        let current = &project.active().clips[0];
        assert_eq!(current.start, 2.0);
        assert_eq!(current.duration, 5.0);
        assert_eq!(current.source_start, 9.0);
        assert_eq!(project.active().clips[1].start, 7.0);
        assert_eq!(project.active().clips[2].start, 10.0);
        assert_eq!(playhead, 2.0);
    }
}
