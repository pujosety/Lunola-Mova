"""Native removal routing source contracts; not GUI interaction evidence."""
from pathlib import Path
import re
import unittest
ROOT = Path(__file__).resolve().parents[2]
class Routing(unittest.TestCase):
    def test_inspector_uses_one_atomic_mode_callback(self):
        panel = (ROOT/'src/crates/concat/ui/inspector/clip-inspector.slint').read_text(encoding='utf-8')
        body = panel.split('function remove-by(index: int) {',1)[1].split('\n    }',1)[0]
        self.assertIn('root.cutout-mode(index);',body)
        self.assertNotIn('chain-remove',body)
        self.assertNotIn('chain-add',body)
    def test_native_projects_full_chain_enabled_keyer_slot(self):
        panel=(ROOT/'src/crates/concat/ui/inspector/clip-inspector.slint').read_text(encoding='utf-8')
        studio=(ROOT/'src/crates/concat/src/studio.rs').read_text(encoding='utf-8')
        self.assertIn('root.selected.removal-keyer-slot - 1',panel)
        self.assertNotIn('pure function keyer-at',panel)
        self.assertIn('Command::SetClipRemovalMode',studio)
        self.assertIn('removal_keyer_slot:',studio)
    def test_dedicated_cutout_section_preserves_basic_panel(self):
        panel=(ROOT/'src/crates/concat/ui/inspector/clip-inspector.slint').read_text(encoding='utf-8')
        config=(ROOT/'src/crates/concat/ui/workspace/config-pane.slint').read_text(encoding='utf-8')
        self.assertTrue('"Basic", "Enhance", "Adjust", "Cutout"' in config)
        self.assertTrue('cutout-only: root.showing == "Cutout";' in config)
        self.assertTrue('if !root.cutout-only: Section' in panel)
        self.assertTrue('if !root.figure && root.show-cutout: Section' in panel)
    def test_background_controls_use_debounced_echo_for_undo(self):
        panel=(ROOT/'src/crates/concat/ui/inspector/clip-inspector.slint').read_text(encoding='utf-8')
        studio=(ROOT/'src/crates/concat/src/studio.rs').read_text(encoding='utf-8')
        clip_set=studio.split('pub fn clip_set(',1)[1].split('pub fn clip_set_text',1)[0]
        self.assertRegex(clip_set, r'cutout_field_blocked\(\s*field,\s*&ids')
        guard = studio.split('fn cutout_field_blocked(', 1)[1].split('\n    }', 1)[0]
        for field in (
            'CutoutFeather',
            'CutoutBackgroundBlur',
            'CutoutExpansion',
            'CutoutEdgeStrength',
            'CutoutEdgeRadius',
        ):
            self.assertIn(f'ClipField::{field}', guard)
        self.assertRegex(guard, r'ids\s*\.iter\(\)\s*\.any\(')
        self.assertIn('clip_track(id)', guard)
        self.assertIn('locked(&track)', guard)
        self.assertIn('root.set(ClipField.cutout-background-blur, v);', panel)
        color=studio.split('pub fn cutout_background_color(',1)[1].split('\n    }',1)[0]
        self.assertIn('self.echo_clip_mut(&id)', color)
        self.assertIn('self.clip_commit()', color)
        self.assertNotIn('self.apply(Command::SetClipCutout', color)
    def test_spatial_mask_controls_route_through_echo_and_publish(self):
        panel=(ROOT/'src/crates/concat/ui/inspector/clip-inspector.slint').read_text(encoding='utf-8')
        model=(ROOT/'src/crates/concat/ui/timeline/model.slint').read_text(encoding='utf-8')
        studio=(ROOT/'src/crates/concat/src/studio.rs').read_text(encoding='utf-8')
        for field in ('cutout-expansion', 'cutout-edge-strength', 'cutout-edge-radius'):
            self.assertTrue(field in model, f'{field} missing from ClipField/SelectedClipData')
            self.assertTrue(f'ClipField.{field}' in panel, f'{field} missing from inspector')
            suffix=field.removeprefix('cutout-').replace('-', '_')
            self.assertTrue(f'cutout_{suffix}:' in studio, f'{field} missing from Rust publication')
        self.assertIn('ClipField::CutoutExpansion', studio)
        self.assertIn('ClipField::CutoutEdgeStrength', studio)
        self.assertIn('ClipField::CutoutEdgeRadius', studio)
        self.assertIn('I18n.t("clipInspector.trackBelowReplacement")', panel)

if __name__=='__main__': unittest.main(verbosity=2)
