#!/usr/bin/env python3
"""Source contract for cross-project clip clipboard wiring (not GUI QA)."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
STUDIO = ROOT / "src/crates/concat/src/studio.rs"


def between(source: str, start: str, end: str) -> str:
    first = source.find(start)
    if first < 0:
        return ""
    last = source.find(end, first + len(start))
    return source[first:] if last < 0 else source[first:last]


class ClipCopyWiring(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = STUDIO.read_text(encoding="utf-8")

    def test_clipboard_records_referenced_media(self):
        held = between(self.source, "pub struct Held {", "/// Where each held clip lands")
        copy = between(self.source, "pub fn copy_clips(", "pub fn paste_clips(")
        self.assertTrue("media: Vec<MediaItem>" in held, "clipboard must carry source metadata")
        self.assertTrue("clip.media_ids()" in copy, "capture every clip media reference")
        self.assertTrue("Some(Held { row, clip, media })" in copy, "store the referenced records in Held")

    def test_paste_rebinds_media_before_placing_each_clip(self):
        paste = between(self.source, "pub fn paste_clips(", "fn held_here(")
        self.assertTrue("self.held_here(held)" in paste, "paste must bind media to the target project")
        self.assertTrue("self.place_copy(&source" in paste, "place the remapped clip")
        self.assertTrue("complete_paste_plan(" in paste, "do not silently paste only part of the group")

    def test_failed_paste_rolls_back_partial_media_and_clips(self):
        paste = between(self.source, "pub fn paste_clips(", "fn held_here(")
        self.assertTrue("let mut failed = false" in paste, "detect a refused metadata or clip edit")
        self.assertTrue(
            "rollback_gesture_if_project_changed(&before)" in paste,
            "undo all earlier edits from this paste after a later command is refused",
        )

    def test_held_media_is_reused_or_added_then_all_ids_are_remapped(self):
        bind = between(self.source, "fn held_here(", "fn place_copy(")
        self.assertTrue("held_media_resolution" in bind, "reuse same-path target items only after compatibility checks")
        self.assertTrue("Command::CopyMediaRecord" in bind, "copy metadata with a new local id")
        self.assertTrue("remap_media_ids" in bind, "rewrite main and enhancement references")

    def test_same_path_media_reuse_requires_a_matching_kind(self):
        resolve = between(self.source, "fn held_media_resolution(", "/// Each rewritten link")
        self.assertTrue("same_media_path" in resolve, "match the existing path first")
        self.assertTrue("mine.kind == item.kind" in resolve, "do not bind clips to a conflicting media kind")
        self.assertTrue("HeldMediaResolution::Incompatible" in resolve)

    def test_all_clip_kinds_get_full_properties_after_creation(self):
        place = between(self.source, "fn place_copy(", "fn sound_tools(")
        helper = between(self.source, "fn match_command(", "fn paste_plan(")
        self.assertTrue("Command::MatchClip" in helper, "copy the complete editable clip model")
        self.assertTrue("match_command(source" in place, "apply full properties to every clip kind")
        self.assertTrue("Command::AddTextClip" in place and "Command::AddShapeClip" in place)

    def test_rejected_property_application_is_reported_as_a_failed_placement(self):
        place = between(self.source, "fn place_copy(", "fn sound_tools(")
        checked = between(self.source, "fn apply_in_checked(", "/// Says so when an edit turned")
        self.assertTrue(
            "if !self.apply_in_checked(gesture, match_command(source, &created))" in place,
            "text and shape property-copy refusal must abort placement",
        )
        self.assertTrue(
            "if !self.apply_in_checked(gesture, Command::Batch { commands })" in place,
            "video property batch refusal must abort placement",
        )
        self.assertTrue("apply_within_result(gesture, command).is_ok()" in checked)
        self.assertTrue("apply_result(command).is_ok()" in checked)


if __name__ == "__main__":
    unittest.main()
