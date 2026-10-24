// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors
//! Real Editor integration tests; no models, GPU or synthetic command mocks.
use concat_project::{
    commands::Command,
    doc::{DocumentSettings, to_document},
    editor::Editor,
    model::{AppliedFilter, Clip, ClipKind, Cutout, CutoutMode, MediaItem, MediaKind, Project},
};
use serde_json::json;
use std::sync::Arc;

fn settings() -> DocumentSettings {
    DocumentSettings {
        name: "Removal QA".into(),
        width: 1920,
        height: 1080,
        rate_num: 30,
        rate_den: 1,
    }
}
fn fixture(cutout: Option<Cutout>, effects: Vec<AppliedFilter>) -> Editor {
    let mut project = Project::new();
    project.media.push(MediaItem {
        id: "m1".into(),
        name: "Original".into(),
        path: "original.mp4".into(),
        kind: MediaKind::Video,
        duration: Some(8.0),
        has_audio: true,
        ..Default::default()
    });
    let track = project.active().tracks[0].id.clone();
    let mut clip = Clip::blank("c1", track, ClipKind::Video, "Original", 0.0, 8.0);
    clip.media_id = "m1".into();
    clip.cutout = cutout;
    clip.video_effects = effects;
    clip.extra
        .insert("futureClipField".into(), json!({"opaque":[1,"keep"]}));
    project.active_mut().clips.push(Arc::new(clip));
    Editor::from_document(&to_document(&settings(), &project)).unwrap()
}
fn switch(editor: &mut Editor, mode: &str) -> bool {
    let command = serde_json::from_value::<Command>(
        json!({"op":"setClipRemovalMode", "clipId":"c1", "mode":mode}),
    );
    assert!(
        command.is_ok(),
        "atomic removal command must be recognized: {command:?}"
    );
    editor.apply(command.unwrap()).unwrap().applied
}
fn clip(editor: &Editor) -> &Clip {
    &editor.project().active().clips[0]
}
fn chain() -> Vec<AppliedFilter> {
    let mut effects: Vec<_> = (0..8)
        .map(|i| AppliedFilter::new(format!("future.effect-{i}")))
        .collect();
    let mut key = AppliedFilter::new("concat.chroma-key");
    key.params.extend([
        ("color".into(), 255.0),
        ("similarity".into(), 17.0),
        ("softness".into(), 3.0),
        ("spill".into(), 87.0),
        ("futureKnob".into(), 42.0),
    ]);
    key.set_key("spill", 0.5, 22.0, Default::default());
    effects.push(key);
    effects.push(AppliedFilter::new("concat.luma-key"));
    effects
}
fn custom_cutout() -> Cutout {
    serde_json::from_value(json!({"mode":"custom", "subject":"object", "feather":0.073,
        "expansion":2, "edge_strength":0.4, "edge_radius":3,
        "background_mode":2, "background_blur":0.125, "background_color":3394815,
        "strokes":[{"tool":"smartEraser", "size":0.123, "points":[[0.2,0.8],[0.4,0.6]], "at":2.75}]})).unwrap()
}
#[test]
fn non_visual_or_mismatched_source_is_a_history_preserving_noop_for_every_mode() {
    for (kind, media_kind) in [
        (ClipKind::Audio, MediaKind::Video),
        (ClipKind::Text, MediaKind::Video),
        (ClipKind::Shape, MediaKind::Video),
        (ClipKind::Layer, MediaKind::Video),
        (ClipKind::Video, MediaKind::Audio),
        (ClipKind::Image, MediaKind::Video),
    ] {
        let base = fixture(Some(custom_cutout()), chain());
        let mut project = base.project().clone();
        Arc::make_mut(&mut project.active_mut().clips[0]).kind = kind;
        project.media[0].kind = media_kind;
        let mut editor = Editor::from_document(&to_document(&settings(), &project)).unwrap();
        let before = editor.project().clone();
        for mode in ["off", "auto", "custom", "chroma"] {
            assert!(!switch(&mut editor, mode), "{kind:?}/{media_kind:?}/{mode}");
            assert_eq!(editor.project(), &before);
            assert!(!editor.can_undo());
        }
    }
}
#[test]
fn unsupported_or_conflicting_retained_metadata_refuses_without_losing_unknown_data() {
    for retained in [
        json!({"futureVersion":2}),
        json!({"mode":"custom", "feather":0.02, "futureCutout":true}),
        json!({"mode":"custom", "feather":9.0}),
        json!({"mode":"custom", "strokes":[{"tool":"brush", "size":0.1,"points":[[0.5,0.5]],"futureStroke":true}]}),
    ] {
        for active in [None, Some(custom_cutout())] {
            let base = fixture(active, chain());
            let mut project = base.project().clone();
            Arc::make_mut(&mut project.active_mut().clips[0])
                .extra
                .insert("lunolaRemovalRetainedCutoutV1".into(), retained.clone());
            let mut editor = Editor::from_document(&to_document(&settings(), &project)).unwrap();
            let before = editor.project().clone();
            for mode in ["off", "auto", "custom", "chroma"] {
                let command: Command = serde_json::from_value(
                    json!({"op":"setClipRemovalMode", "clipId":"c1", "mode":mode}),
                )
                .unwrap();
                assert!(
                    editor.apply(command).is_err(),
                    "unsupported metadata must not be consumed or overwritten"
                );
                assert_eq!(editor.project(), &before);
                assert!(!editor.can_undo());
            }
        }
    }
}
#[test]
fn chroma_prefers_enabled_keyer_over_earlier_disabled_duplicate_and_bypasses_other_keyers() {
    let mut effects = chain();
    effects[8].enabled = false;
    let mut existing = AppliedFilter::new("concat.blue-screen");
    existing.params.insert("similarity".into(), 19.0);
    effects.push(existing);
    effects.push(AppliedFilter::new("concat.green-screen"));
    let mut editor = fixture(Some(custom_cutout()), effects.clone());
    let before = editor.project().clone();
    assert!(switch(&mut editor, "chroma"));
    effects[11].enabled = false;
    assert_eq!(clip(&editor).video_effects, effects);
    assert!(clip(&editor).cutout.is_none());
    assert!(!switch(&mut editor, "chroma"));
    let after = editor.project().clone();
    assert!(editor.undo());
    assert_eq!(editor.project(), &before);
    assert!(!editor.can_undo());
    assert!(editor.redo());
    assert_eq!(editor.project(), &after);
    assert!(switch(&mut editor, "off"));
    assert!(
        clip(&editor)
            .video_effects
            .iter()
            .filter(|effect| [
                "concat.chroma-key",
                "concat.green-screen",
                "concat.blue-screen"
            ]
            .contains(&effect.id.as_str()))
            .all(|effect| !effect.enabled)
    );
}
#[test]
fn all_four_modes_roundtrip_with_valid_media_and_retain_full_settings() {
    let mut effects = chain();
    effects[8].enabled = false;
    let mut editor = fixture(Some(custom_cutout()), effects);
    for mode in [
        "off", "auto", "custom", "chroma", "custom", "off", "chroma", "auto", "custom",
    ] {
        let before = editor.project().clone();
        assert!(switch(&mut editor, mode));
        let after = editor.project().clone();
        assert!(!switch(&mut editor, mode));
        assert!(editor.undo());
        assert_eq!(editor.project(), &before);
        // A no-op must not invalidate redo.
        let current = match clip(&editor).cutout.as_ref().map(|mask| mask.mode) {
            Some(CutoutMode::Auto) => "auto",
            Some(CutoutMode::Custom) => "custom",
            None if clip(&editor).video_effects[8].enabled => "chroma",
            None => "off",
        };
        assert!(!switch(&mut editor, current));
        assert!(editor.can_redo());
        assert!(editor.redo());
        assert_eq!(editor.project(), &after);
        let saved = editor.to_document(&settings());
        editor = Editor::from_document(&serde_json::from_str(&saved.to_string()).unwrap()).unwrap();
        assert_eq!(editor.project(), &after);
        assert_eq!(clip(&editor).media_id, "m1");
        assert_eq!(editor.project().media[0].path, "original.mp4");
        assert_eq!(clip(&editor).video_effects[8].params, chain()[8].params);
        assert_eq!(clip(&editor).video_effects[8].keys, chain()[8].keys);
        assert_eq!(clip(&editor).video_effects.len(), 10);
        assert_eq!(
            clip(&editor).extra["futureClipField"],
            json!({"opaque":[1,"keep"]})
        );
        match mode {
            "off" | "chroma" => assert_eq!(
                clip(&editor).extra["lunolaRemovalRetainedCutoutV1"],
                serde_json::to_value(custom_cutout()).unwrap()
            ),
            _ => {
                let mask = clip(&editor).cutout.as_ref().unwrap();
                assert_eq!(mask.strokes, custom_cutout().strokes);
                assert_eq!(mask.feather, custom_cutout().feather);
                assert_eq!(mask.subject, custom_cutout().subject);
                assert_eq!(mask.expansion, custom_cutout().expansion);
                assert_eq!(mask.edge_strength, custom_cutout().edge_strength);
                assert_eq!(mask.edge_radius, custom_cutout().edge_radius);
                assert_eq!(mask.background_mode, custom_cutout().background_mode);
                assert_eq!(mask.background_blur, custom_cutout().background_blur);
                assert_eq!(mask.background_color, custom_cutout().background_color);
            }
        }
    }
}
#[test]
fn fresh_image_modes_use_existing_schema_defaults_and_append_only_one_keyer() {
    let base = fixture(None, vec![]);
    let mut project = base.project().clone();
    Arc::make_mut(&mut project.active_mut().clips[0]).kind = ClipKind::Image;
    project.media[0].kind = MediaKind::Image;
    let mut editor = Editor::from_document(&to_document(&settings(), &project)).unwrap();
    assert!(!switch(&mut editor, "off"));
    assert!(switch(&mut editor, "auto"));
    assert_eq!(clip(&editor).cutout, Some(Cutout::auto()));
    assert!(switch(&mut editor, "custom"));
    assert!(switch(&mut editor, "chroma"));
    assert_eq!(
        clip(&editor).video_effects,
        vec![AppliedFilter::new("concat.chroma-key")]
    );
    assert!(switch(&mut editor, "off"));
    assert!(switch(&mut editor, "chroma"));
    assert_eq!(clip(&editor).video_effects.len(), 1);
    let document = editor.to_document(&settings());
    let loaded = Editor::from_document(&document).unwrap();
    assert_eq!(loaded.project(), editor.project());
    let command: Command = serde_json::from_value(
        json!({"op":"setClipRemovalMode", "clipId":"absent", "mode":"custom"}),
    )
    .unwrap();
    let before = editor.project().clone();
    assert!(!editor.apply(command).unwrap().applied);
    assert_eq!(editor.project(), &before);
}
#[test]
fn custom_off_save_reload_custom_restores_exact_mask_and_source_with_atomic_history() {
    let mut editor = fixture(Some(custom_cutout()), chain());
    let before = editor.project().clone();
    assert!(switch(&mut editor, "off"));
    assert!(
        clip(&editor).cutout.is_none(),
        "Off must bypass a mask, not merely keyers"
    );
    assert_eq!(
        clip(&editor).extra["lunolaRemovalRetainedCutoutV1"],
        serde_json::to_value(custom_cutout()).unwrap()
    );
    assert!(!switch(&mut editor, "off"));
    let off = editor.project().clone();
    assert!(editor.undo());
    assert_eq!(editor.project(), &before);
    assert!(!editor.can_undo());
    assert!(editor.redo());
    assert_eq!(editor.project(), &off);
    let saved = editor.to_document(&settings());
    let mut loaded =
        Editor::from_document(&serde_json::from_str(&saved.to_string()).unwrap()).unwrap();
    assert_eq!(loaded.project(), &off);
    assert!(!loaded.can_undo());
    assert!(switch(&mut loaded, "custom"));
    assert_eq!(clip(&loaded).cutout, Some(custom_cutout()));
    assert!(
        !clip(&loaded)
            .extra
            .contains_key("lunolaRemovalRetainedCutoutV1")
    );
    assert_eq!(clip(&loaded).media_id, "m1");
    assert_eq!(loaded.project().media, before.media);
    assert_eq!(clip(&loaded).extra, before.active().clips[0].extra);
    assert!(!switch(&mut loaded, "custom"));
    assert!(loaded.undo());
    assert_eq!(loaded.project(), &off);
    assert!(!loaded.can_undo());
    assert!(loaded.redo());
    assert_eq!(clip(&loaded).cutout, Some(custom_cutout()));
    assert!(switch(&mut loaded, "auto"));
    let mut automatic = custom_cutout();
    automatic.mode = CutoutMode::Auto;
    assert_eq!(clip(&loaded).cutout, Some(automatic));
    assert!(switch(&mut loaded, "custom"));
    assert_eq!(clip(&loaded).cutout, Some(custom_cutout()));
}
#[test]
fn chroma_reenables_existing_disabled_keyer_at_index_eight_with_exact_settings() {
    let mut effects = chain();
    effects[8].enabled = false;
    let mut editor = fixture(None, effects);
    let before = editor.project().clone();
    assert!(switch(&mut editor, "chroma"));
    assert_eq!(clip(&editor).video_effects, chain());
    assert_eq!(clip(&editor).media_id, "m1");
    assert!(!switch(&mut editor, "chroma"));
    assert!(editor.undo());
    assert_eq!(editor.project(), &before);
    assert!(!editor.can_undo());
    assert!(editor.redo());
    assert_eq!(clip(&editor).video_effects, chain());
}
#[test]
fn off_bypasses_index_eight_keyer_without_losing_chain_and_is_one_undo_step() {
    let mut editor = fixture(None, chain());
    let before = editor.project().clone();
    assert!(switch(&mut editor, "off"));
    let mut expected = before.active().clips[0].video_effects.clone();
    expected[8].enabled = false;
    assert_eq!(clip(&editor).video_effects, expected);
    assert_eq!(clip(&editor).extra, before.active().clips[0].extra);
    let after = editor.project().clone();
    assert!(!switch(&mut editor, "off"));
    assert!(editor.undo());
    assert_eq!(editor.project(), &before);
    assert!(!editor.can_undo());
    assert!(editor.redo());
    assert_eq!(editor.project(), &after);
    assert!(!editor.can_redo());
}
