# SPDX-License-Identifier: AGPL-3.0-or-later
"""Source contracts for preview/edit-cursor wiring, not real GUI proof."""
from pathlib import Path
import re
import unittest
ROOT = Path(__file__).resolve().parents[2]

def method(text, name):
    start = text.index('    pub fn ' + name + '(')
    end = text.find('\n    pub fn ', start + 1)
    return re.sub(r'\s+', ' ', text[start:end if end >= 0 else None])

class ScrubWiring(unittest.TestCase):
    def test_timeline_hover_updates_the_real_edit_cursor(self):
        studio = (ROOT/'src/crates/concat/src/studio.rs').read_text(encoding='utf-8')
        hover = method(studio, 'hover')
        self.assertTrue('crate::timeline_cursor::scrub(' in hover, 'Hover must update the edit cursor via the tested reducer')
        self.assertTrue('&mut self.playhead, &mut self.hover' in hover)
        self.assertTrue('!self.prefs.preview_axis || self.playing' in hover)
        self.assertTrue('self.close_source();' in hover)
    def test_explicit_seek_clears_hover_and_cancels_stale_audio_seek(self):
        studio = (ROOT/'src/crates/concat/src/studio.rs').read_text(encoding='utf-8')
        seek = method(studio, 'seek')
        self.assertTrue('self.hover.take()' in seek)
        self.assertTrue('self.hover_hush.stop();' in seek)
        hover = method(studio, 'hover')
        self.assertTrue('studio.host.playback.seek(f64::from(studio.playhead));' in hover)
    def test_the_main_line_replaces_the_old_second_hover_line(self):
        lanes = (ROOT/'src/crates/concat/ui/timeline/lanes.slint').read_text(encoding='utf-8')
        self.assertFalse('if root.preview-axis && root.hover-on: Rectangle' in lanes)
        self.assertTrue('x: root.at(root.playhead);' in lanes)
        self.assertTrue('if root.tool == TimelineTool.split && root.hover-on: Rectangle' in lanes)

if __name__ == '__main__': unittest.main(verbosity=2)
