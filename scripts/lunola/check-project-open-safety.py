# SPDX-License-Identifier: AGPL-3.0-or-later
"""Source-order contract only; not a live project lifecycle test."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
STUDIO = ROOT / 'src/crates/concat/src/studio.rs'


def section(source: str, start: str, end: str) -> str:
    left = source.find(start)
    if left < 0:
        return ''
    right = source.find(end, left + len(start))
    return source[left:right] if right >= 0 else ''


class ProjectOpenSafety(unittest.TestCase):
    def test_open_claims_the_new_folder_before_saving_and_releasing_the_old(self):
        source = STUDIO.read_text(encoding='utf-8')
        body = section(source, 'pub fn open_project(', 'pub fn close_project(')
        same = body.find('same_folder(')
        claim = body.find('.claim_then(')
        save = body.find('save_and_release()')
        self.assertGreaterEqual(same, 0, 'opening the already-open folder must be a no-op')
        self.assertGreaterEqual(claim, 0, 'claim the new project before touching the current one')
        self.assertGreaterEqual(save, 0, 'the open session must save and release before switching')
        self.assertLess(same, claim)
        self.assertLess(claim, save)
        self.assertIn('ClaimThenError::HeldBy', body)
        self.assertIn('ClaimThenError::Preparation', body)

    def test_open_validates_the_target_before_closing_the_current_session(self):
        source = STUDIO.read_text(encoding='utf-8')
        body = section(source, 'pub fn open_project(', 'pub fn close_project(')
        claim = body.find('.claim_then(')
        open_target = body.find('Session::open_info(&info)')
        save_current = body.find('self.save_and_release()')
        self.assertGreaterEqual(open_target, 0, 'parse the destination before switching')
        self.assertLess(claim, open_target)
        self.assertLess(open_target, save_current)

    def test_close_and_switch_teardown_happens_only_after_a_successful_save(self):
        source = STUDIO.read_text(encoding='utf-8')
        close = section(source, 'pub fn close_project(', 'fn save_and_release(')
        release = section(source, 'fn save_and_release(', '/// Posters for the recents')
        self.assertIn('self.save_and_release()', close)
        save_positions = [
            release.find('projects::save_ordered('),
            release.find('projects::save('),
        ]
        save = min((position for position in save_positions if position >= 0), default=-1)
        stop = release.find('self.autosave.stop()')
        unlock = release.find('self.host.open_projects.release(')
        forget = release.find('self.session = None')
        self.assertGreaterEqual(save, 0, 'the old project must be written')
        self.assertGreaterEqual(stop, 0)
        self.assertGreaterEqual(unlock, 0)
        self.assertGreaterEqual(forget, 0)
        self.assertLess(save, stop)
        self.assertLess(stop, unlock)
        self.assertLess(unlock, forget)

    def test_window_and_installer_exit_paths_are_gated_on_a_successful_close(self):
        lib = (ROOT / 'src/crates/concat/src/lib.rs').read_text(encoding='utf-8')
        settings = (ROOT / 'src/crates/concat/src/panes/settings.rs').read_text(encoding='utf-8')
        studio = STUDIO.read_text(encoding='utf-8')
        self.assertTrue('pub(crate) fn close_window(' in lib, 'close needs one shared save gate')
        self.assertTrue(
            'CloseRequestResponse::KeepWindowShown' in lib,
            'a failed system close must leave the window visible',
        )
        self.assertTrue(
            re.search(r'pub fn close_project\(&mut self\) -> Result<\(\), String>', studio),
            'close_project must report save failure to its caller',
        )
        self.assertTrue(
            'close_window(' in settings,
            'version switching must not outrun a failed save',
        )


if __name__ == '__main__':
    unittest.main(verbosity=2)
