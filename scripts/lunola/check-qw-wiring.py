# SPDX-License-Identifier: AGPL-3.0-or-later
"""Source contract only; this does not claim real keyboard/GUI validation."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]

class QWBindingContract(unittest.TestCase):
    def test_plain_qw_reach_the_ripple_adapter_after_modifier_guards(self):
        app = (ROOT / 'src/crates/concat/ui/app.slint').read_text(encoding='utf-8')
        studio = (ROOT / 'src/crates/concat/src/studio.rs').read_text(encoding='utf-8')
        for key, action, edge in [('q', 'ripple-trim-start', 'Start'), ('w', 'ripple-trim-end', 'End')]:
            binding = f'if (letter == "{key}") {{ Editor.shortcut("{action}"); return accept; }}'
            self.assertTrue(binding in app, f'Missing {key.upper()} -> {action} binding')
            self.assertLess(app.index('if (event.modifiers.shift)'), app.index(binding))
            self.assertLess(app.index('if (event.modifiers.control || event.modifiers.meta)'), app.index(binding))
            self.assertLess(app.index('if (event.modifiers.alt)'), app.index(binding))
            self.assertRegex(studio, re.escape(f'"{action}"') + r'\s*=>\s*self\.ripple_trim_at_playhead\(TrimEdge::' + edge + r'\)')
        self.assertIn('crate::ripple_trim::plan(', studio)

    def test_qw_are_blocked_while_a_modal_or_menu_is_open(self):
        app = (ROOT / 'src/crates/concat/ui/app.slint').read_text(encoding='utf-8')
        guard = 'root.sheet-open || root.confirm.open || root.relink.open || root.open-menu >= 0'
        self.assertTrue(guard in app, 'Q/W need a modal/menu guard before destructive editing')
        self.assertLess(app.index(guard), app.index('if (letter == "q")'))

    def test_shortcut_focus_returns_after_every_blocking_surface_closes(self):
        app = (ROOT / 'src/crates/concat/ui/app.slint').read_text(encoding='utf-8')
        self.assertIn('forward-focus: keys;', app)
        self.assertRegex(app, r'changed on-start => \{\s*if \(!root\.on-start\) \{\s*keys\.focus\(\);')
        sheet = re.search(r'property <bool> sheet-open:(.*?);', app, re.S)
        self.assertIsNotNone(sheet, 'Missing shared focus-restoration state')
        for surface in ('root.confirm.open', 'root.relink.open', 'root.open-menu >= 0'):
            self.assertIn(surface, sheet.group(1), f'{surface} must restore the window shortcut scope on close')
        self.assertRegex(app, r'changed sheet-open => \{\s*if \(!root\.sheet-open\) \{\s*keys\.focus\(\);')

if __name__ == '__main__':
    unittest.main(verbosity=2)
