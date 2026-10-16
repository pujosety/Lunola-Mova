// SPDX-License-Identifier: AGPL-3.0-or-later
//! Real-engine regression for Lunola's separated AV timeline workflow.
use concat_project::{
    commands::Command,
    doc::{DocumentSettings, to_document},
    editor::Editor,
    model::{ClipKind, MediaItem, MediaKind, Project},
};
use serde_json::json;

fn editor() -> Editor {
    let mut project = Project::new();
    project.media.push(MediaItem {
        id: "m1".into(),
        name: "Camera".into(),
        path: "camera.mp4".into(),
        duration: Some(8.0),
        kind: MediaKind::Video,
        has_audio: true,
        ..Default::default()
    });
    Editor::from_document(&to_document(
        &DocumentSettings {
            name: "AV QA".into(),
            width: 1920,
            height: 1080,
            rate_num: 30,
            rate_den: 1,
        },
        &project,
    ))
    .unwrap()
}

#[path = "../../concat/src/timeline_av.rs"]
mod timeline_av;

#[path = "../../concat/src/ripple_trim.rs"]
mod ripple_trim;

#[path = "../../concat/src/timeline_cursor.rs"]
mod timeline_cursor;

#[test]
fn hovering_then_q_cuts_the_previewed_frame_not_the_old_cursor() {
    let mut editor = editor();
    let add: Command =
        serde_json::from_value(json!({"op":"addClipWithAudio", "mediaId":"m1", "start":0.0}))
            .unwrap();
    let id = editor.apply(add).unwrap().created_id.unwrap();
    let mut playhead = 0.0;
    let mut hover = None;
    assert!(timeline_cursor::scrub(
        &mut playhead,
        &mut hover,
        3.0,
        8.0,
        false
    ));
    assert_eq!(
        playhead,
        hover.unwrap(),
        "the main line must match the preview"
    );
    let (command, _) = ripple_trim::plan(
        editor.project().active(),
        &[id],
        f64::from(playhead),
        concat_project::commands::TrimEdge::Start,
        |_| false,
    )
    .expect("Q should use the hovered position");
    editor.apply(command).unwrap();
    assert!(
        editor
            .project()
            .active()
            .clips
            .iter()
            .all(|c| c.duration == 5.0 && c.source_start == 3.0)
    );
}

#[test]
fn hovering_then_w_uses_the_preview_position_for_video_and_audio() {
    let mut editor = editor();
    let add: Command =
        serde_json::from_value(json!({"op":"addClipWithAudio", "mediaId":"m1", "start":0.0}))
            .unwrap();
    let id = editor.apply(add).unwrap().created_id.unwrap();
    let mut playhead = 0.0;
    let mut hover = None;
    timeline_cursor::scrub(&mut playhead, &mut hover, 5.0, 8.0, false);
    let (command, at) = ripple_trim::plan(
        editor.project().active(),
        &[id],
        f64::from(playhead),
        concat_project::commands::TrimEdge::End,
        |_| false,
    )
    .unwrap();
    editor.apply(command).unwrap();
    assert_eq!(at, 5.0);
    assert!(
        editor
            .project()
            .active()
            .clips
            .iter()
            .all(|c| c.duration == 5.0 && c.source_start == 0.0)
    );
}

#[test]
fn scrub_clamps_consistently_ignores_invalid_input_and_skips_duplicate_redraws() {
    let mut playhead = 0.0;
    let mut hover = None;
    assert!(timeline_cursor::scrub(
        &mut playhead,
        &mut hover,
        12.0,
        8.0,
        true
    ));
    assert_eq!((playhead, hover), (8.0, Some(8.0)));
    assert!(!timeline_cursor::scrub(
        &mut playhead,
        &mut hover,
        12.0,
        8.0,
        true
    ));
    for bad in [f32::NAN, f32::INFINITY] {
        assert!(!timeline_cursor::scrub(
            &mut playhead,
            &mut hover,
            bad,
            8.0,
            false
        ));
        assert_eq!((playhead, hover), (8.0, Some(8.0)));
    }
    assert!(timeline_cursor::scrub(
        &mut playhead,
        &mut hover,
        -1.0,
        8.0,
        false
    ));
    assert_eq!((playhead, hover), (0.0, Some(0.0)));
    assert!(timeline_cursor::scrub(
        &mut playhead,
        &mut hover,
        12.0,
        8.0,
        false
    ));
    assert_eq!((playhead, hover), (12.0, Some(12.0)));
}

#[test]
fn all_encoded_audio_streams_remain_separate_and_round_trip() {
    use concat_project::model::AudioTrack;
    let mut project = Project::new();
    let stream = |index| AudioTrack {
        index,
        codec: "aac".into(),
        channels: 2,
        sample_rate: 48000,
        title: format!("Stream {index}"),
        language: "".into(),
    };
    project.media.push(MediaItem {
        id: "m1".into(),
        name: "Camera".into(),
        path: "multistream.mp4".into(),
        duration: Some(8.0),
        kind: MediaKind::Video,
        has_audio: true,
        audio_tracks: vec![stream(1), stream(2)],
        ..Default::default()
    });
    let settings = DocumentSettings {
        name: "Multistream".into(),
        width: 1920,
        height: 1080,
        rate_num: 30,
        rate_den: 1,
    };
    let mut editor = Editor::from_document(&to_document(&settings, &project)).unwrap();
    let add: Command =
        serde_json::from_value(json!({"op":"addClipWithAudio", "mediaId":"m1", "start":0.0}))
            .unwrap();
    let id = editor.apply(add).unwrap().created_id.unwrap();
    let timeline = editor.project().active();
    assert_eq!(
        timeline_av::targets(timeline, &[id], |_| false)
            .unwrap()
            .len(),
        3
    );
    let sounds: Vec<_> = timeline
        .clips
        .iter()
        .filter(|c| c.kind == ClipKind::Audio)
        .collect();
    assert_eq!(sounds.len(), 2);
    assert_ne!(sounds[0].track_id, sounds[1].track_id);
    assert_eq!(
        sounds.iter().map(|c| c.audio_stream).collect::<Vec<_>>(),
        vec![Some(1), Some(2)]
    );
    let reopened = Editor::from_document(&editor.to_document(&settings)).unwrap();
    assert_eq!(reopened.project(), editor.project());
}

#[test]
fn a_locked_partner_blocks_qw_and_pointer_timing_without_partial_edits() {
    let mut editor = editor();
    let add: Command =
        serde_json::from_value(json!({"op":"addClipWithAudio", "mediaId":"m1", "start":0.0}))
            .unwrap();
    let id = editor.apply(add).unwrap().created_id.unwrap();
    let timeline = editor.project().active();
    let audio_track = timeline
        .clips
        .iter()
        .find(|c| c.kind == ClipKind::Audio)
        .unwrap()
        .track_id
        .clone();
    assert!(
        ripple_trim::plan(
            timeline,
            &[id.clone()],
            3.0,
            concat_project::commands::TrimEdge::Start,
            |track| track == audio_track
        )
        .is_none()
    );
    assert!(
        timeline_av::trim_plan(
            timeline,
            &id,
            concat_project::commands::TrimEdge::End,
            -2.0,
            true,
            |track| track == audio_track
        )
        .is_none()
    );
}

#[test]
fn a_silent_video_stays_a_single_clip() {
    let mut project = Project::new();
    project.media.push(MediaItem {
        id: "silent".into(),
        path: "silent.mp4".into(),
        kind: MediaKind::Video,
        duration: Some(8.0),
        has_audio: false,
        ..Default::default()
    });
    let settings = DocumentSettings {
        name: "Silent".into(),
        width: 1920,
        height: 1080,
        rate_num: 30,
        rate_den: 1,
    };
    let mut editor = Editor::from_document(&to_document(&settings, &project)).unwrap();
    let add: Command =
        serde_json::from_value(json!({"op":"addClipWithAudio", "mediaId":"silent", "start":0.0}))
            .unwrap();
    editor.apply(add).unwrap();
    assert_eq!(editor.project().active().clips.len(), 1);
    assert_ne!(editor.project().active().clips[0].muted, Some(true));
}

#[test]
fn new_sound_does_not_reuse_a_muted_audio_lane() {
    let mut editor = editor();
    let add = || {
        serde_json::from_value::<Command>(
            json!({"op":"addClipWithAudio", "mediaId":"m1", "start":0.0}),
        )
        .unwrap()
    };
    editor.apply(add()).unwrap();
    let muted = editor
        .project()
        .active()
        .clips
        .iter()
        .find(|c| c.kind == ClipKind::Audio)
        .unwrap()
        .track_id
        .clone();
    editor
        .apply(Command::SetTrackFlag {
            track_id: muted.clone(),
            flag: concat_project::commands::TrackFlag::Muted,
            value: true,
        })
        .unwrap();
    let next: Command =
        serde_json::from_value(json!({"op":"addClipWithAudio", "mediaId":"m1", "start":8.0}))
            .unwrap();
    let video = editor.apply(next).unwrap().created_id.unwrap();
    let timeline = editor.project().active();
    let audio = timeline
        .clips
        .iter()
        .find(|c| c.detached_from.as_deref() == Some(&video))
        .unwrap();
    assert!(
        !timeline.track(&audio.track_id).unwrap().muted,
        "new AV must not become silently muted by lane reuse"
    );
}

#[test]
fn separate_lanes_have_honest_video_and_audio_labels() {
    let mut editor = editor();
    let add: Command =
        serde_json::from_value(json!({"op":"addClipWithAudio", "mediaId":"m1", "start":0.0}))
            .unwrap();
    editor.apply(add).unwrap();
    let timeline = editor.project().active();
    for clip in &timeline.clips {
        let (label, audio) = timeline_av::lane_label(timeline, &clip.track_id);
        assert_eq!(audio, clip.kind == ClipKind::Audio);
        assert_eq!(label, if audio { "A1" } else { "V1" });
    }
}

#[test]
fn pointer_trim_uses_the_same_delta_for_linked_sound_and_one_undo_step() {
    let mut editor = editor();
    let add: Command =
        serde_json::from_value(json!({"op":"addClipWithAudio", "mediaId":"m1", "start":0.0}))
            .unwrap();
    let id = editor.apply(add).unwrap().created_id.unwrap();
    let before = editor.project().clone();
    let command = timeline_av::trim_plan(
        editor.project().active(),
        &id,
        concat_project::commands::TrimEdge::Start,
        2.0,
        true,
        |_| false,
    )
    .expect("pointer trim needs an AV plan");
    editor.apply(command).unwrap();
    assert!(
        editor
            .project()
            .active()
            .clips
            .iter()
            .all(|c| c.duration == 6.0 && c.source_start == 2.0 && c.start == 0.0)
    );
    assert!(editor.undo());
    assert_eq!(editor.project(), &before);
}

#[test]
fn q_trims_linked_sound_when_only_video_is_selected() {
    let mut editor = editor();
    let add: Command =
        serde_json::from_value(json!({"op":"addClipWithAudio", "mediaId":"m1", "start":0.0}))
            .unwrap();
    let id = editor.apply(add).unwrap().created_id.unwrap();
    let (command, _) = ripple_trim::plan(
        editor.project().active(),
        &[id],
        3.0,
        concat_project::commands::TrimEdge::Start,
        |_| false,
    )
    .unwrap();
    editor.apply(command).unwrap();
    assert!(
        editor
            .project()
            .active()
            .clips
            .iter()
            .all(|clip| clip.duration == 5.0 && clip.source_start == 3.0),
        "Q must preserve AV source sync"
    );
}

#[test]
fn linked_timing_targets_include_sound_but_unlink_restores_independent_editing() {
    let mut editor = editor();
    let add: Command =
        serde_json::from_value(json!({"op":"addClipWithAudio", "mediaId":"m1", "start":0.0}))
            .unwrap();
    let id = editor.apply(add).unwrap().created_id.unwrap();
    let targets =
        timeline_av::targets(editor.project().active(), &[id.clone()], |_| false).unwrap();
    assert_eq!(targets.len(), 2, "linked timing must include audio");
    let unlink: Command =
        serde_json::from_value(json!({"op":"setClipAvLinked", "clipId":id, "linked":false}))
            .unwrap();
    editor.apply(unlink).unwrap();
    assert_eq!(
        timeline_av::targets(editor.project().active(), &[id.clone()], |_| false).unwrap(),
        vec![id]
    );
}

#[test]
fn inserting_before_linked_video_moves_its_audio_by_the_same_amount() {
    let mut editor = editor();
    let first: Command = serde_json::from_value(
        json!({"op":"addClipWithAudio", "mediaId":"m1", "start":0.0, "ripple":true}),
    )
    .unwrap();
    let original = editor.apply(first).unwrap().created_id.unwrap();
    let second: Command = serde_json::from_value(
        json!({"op":"addClipWithAudio", "mediaId":"m1", "start":0.0, "ripple":true}),
    )
    .unwrap();
    editor.apply(second).unwrap();
    let timeline = editor.project().active();
    let video = timeline.clip(&original).unwrap();
    let audio = timeline
        .clips
        .iter()
        .find(|c| c.detached_from.as_deref() == Some(&original))
        .unwrap();
    assert_eq!(video.start, 8.0);
    assert_eq!(
        audio.start, video.start,
        "insertion must not break existing AV sync"
    );
}

#[test]
fn explicit_unlink_keeps_sound_independent_and_can_be_undone() {
    let mut editor = editor();
    let add: Command =
        serde_json::from_value(json!({"op":"addClipWithAudio", "mediaId":"m1", "start":0.0}))
            .unwrap();
    let video = editor.apply(add).unwrap().created_id.unwrap();
    let unlink: Command =
        serde_json::from_value(json!({"op":"setClipAvLinked", "clipId":video, "linked":false}))
            .expect("AV needs an explicit unlink operation");
    editor.apply(unlink).unwrap();
    assert!(
        editor
            .project()
            .active()
            .clips
            .iter()
            .all(|c| c.extra.get("lunolaAvLinked") == Some(&json!(false)))
    );
    assert!(editor.undo());
    assert!(
        editor
            .project()
            .active()
            .clips
            .iter()
            .all(|c| c.extra.get("lunolaAvLinked") == Some(&json!(true)))
    );
}

#[test]
fn adding_a_video_separates_sound_below_it_as_one_undoable_edit() {
    let mut editor = editor();
    let before = editor.project().clone();
    let command: Command = serde_json::from_value(
        json!({"op":"addClipWithAudio", "mediaId":"m1", "start":2.0, "ripple":true}),
    )
    .expect("Mova needs an atomic AV insertion command");
    let result = editor.apply(command).unwrap();
    let timeline = editor.project().active();
    assert_eq!(timeline.clips.len(), 2);
    let video = timeline
        .clips
        .iter()
        .find(|c| c.kind == ClipKind::Video)
        .unwrap();
    let sound = timeline
        .clips
        .iter()
        .find(|c| c.kind == ClipKind::Audio)
        .unwrap();
    assert_eq!(result.created_id.as_deref(), Some(video.id.as_str()));
    assert_eq!(
        video.muted,
        Some(true),
        "do not play embedded and detached audio together"
    );
    assert_eq!(sound.detached_from.as_deref(), Some(video.id.as_str()));
    assert_eq!(
        (sound.start, sound.duration, sound.source_start),
        (video.start, video.duration, video.source_start)
    );
    let row = |id: &str| timeline.tracks.iter().position(|t| t.id == id).unwrap();
    assert!(
        row(&sound.track_id) < row(&video.track_id),
        "audio must appear below video in the reversed UI stack"
    );
    assert_eq!(
        sound.extra.get("lunolaAvGroup"),
        video.extra.get("lunolaAvGroup")
    );
    assert_eq!(sound.extra.get("lunolaAvLinked"), Some(&json!(true)));
    assert!(editor.undo());
    assert_eq!(editor.project(), &before);
    assert!(!editor.undo());
    assert!(editor.redo());
    assert_eq!(editor.project().active().clips.len(), 2);
}
