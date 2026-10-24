# SPDX-License-Identifier: AGPL-3.0-or-later
"""Source wiring contracts; not GUI keystroke/drag proof."""
from pathlib import Path
import re
import unittest
ROOT = Path(__file__).resolve().parents[2]
class AVWiring(unittest.TestCase):
    def test_media_bin_and_timeline_headers_use_the_av_workflow(self):
        studio = (ROOT/'src/crates/concat/src/studio.rs').read_text(encoding='utf-8')
        self.assertGreaterEqual(studio.count('Command::AddClipWithAudio'), 2)
        self.assertIn('crate::timeline_av::lane_label(', studio)
        self.assertIn('label: label.into()', studio)
        model = (ROOT/'src/crates/concat/ui/timeline/model.slint').read_text(encoding='utf-8')
        self.assertIn('label: string,', model)
        header = (ROOT/'src/crates/concat/ui/timeline/track-header.slint').read_text(encoding='utf-8')
        self.assertIn('text: root.track.label;', header)
    def test_link_controls_and_timing_adapters_are_wired(self):
        studio = (ROOT/'src/crates/concat/src/studio.rs').read_text(encoding='utf-8')
        for marker in ['"unlink-av"', '"link-av"', 'Command::SetClipAvLinked', 'crate::timeline_av::trim_plan(', 'crate::timeline_av::targets(']:
            self.assertTrue(marker in studio, f'Missing AV wiring: {marker}')
        self.assertGreaterEqual(studio.count('crate::timeline_av::targets('), 4)
    def test_linked_speed_and_video_mute_keep_audio_in_the_edit(self):
        studio = (ROOT/'src/crates/concat/src/studio.rs').read_text(encoding='utf-8')
        self.assertIn('field == ClipField::Speed || field == ClipField::Volume', studio)
        self.assertRegex(studio, r'let mute_ids\s*=\s*crate::timeline_av::targets\(')

if __name__ == '__main__': unittest.main(verbosity=2)
