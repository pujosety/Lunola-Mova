# SPDX-License-Identifier: AGPL-3.0-or-later
"""Source/resource regression contracts; not a substitute for native GUI tests."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class BrandLayoutContracts(unittest.TestCase):
    def test_splash_art_is_anchored_to_hero_origin_not_implicitly_centered(self):
        source = (ROOT / 'src/crates/concat/ui/brand.slint').read_text(encoding='utf-8')
        hero = source.split('export component StartHero inherits Rectangle {', 1)[1]
        art = hero.split('    Rectangle {', 1)[1].split('        Image {', 1)[0]
        self.assertRegex(art, r'\bx:\s*0px\s*;', 'Splash needs explicit left origin: otherwise Slint centres it into the identity panel')
        self.assertRegex(art, r'\by:\s*0px\s*;', 'Splash needs explicit top origin for the stacked layout')


if __name__ == '__main__':
    unittest.main(verbosity=2)
