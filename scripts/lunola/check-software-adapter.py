#!/usr/bin/env python3
"""Source contract only; does not exercise the GPU or Slint renderer."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
PLATFORM = ROOT / "src/crates/concat/src/platform.rs"


def section(source: str, start: str, end: str) -> str:
    left = source.find(start)
    if left < 0:
        return ""
    right = source.find(end, left + len(start))
    return source[left:right] if right >= 0 else ""


class SoftwareAdapterContract(unittest.TestCase):
    def test_warp_is_not_shared_with_the_slint_window(self):
        source = PLATFORM.read_text(encoding="utf-8")
        select = section(source, "pub fn select_backend(", "selector.select()?")
        self.assertTrue("let mut gpu = Gpu::acquire()" in select)
        self.assertTrue("let software = gpu.as_ref().is_some_and(Gpu::is_software)" in select)
        self.assertTrue('std::env::set_var("CONCAT_GPU", "software")' in select)
        self.assertTrue("gpu = None" in select)
        self.assertTrue('selector.renderer_name("skia-software".into())' in select)
        self.assertFalse('std::env::set_var("SLINT_WGPU_CPU", "1")' in select)

    def test_monitor_honors_the_platform_software_only_request(self):
        source = (ROOT / "src/crates/concat-render/src/gpu.rs").read_text(encoding="utf-8")
        policy = section(source, "fn own_api_first(software_only: bool)", "fn on_backends(")
        self.assertTrue('std::env::var("CONCAT_GPU")' in policy)
        self.assertIsNotNone(re.search(r"software_only = software_only\s*\|\|", policy))
        self.assertTrue("force_fallback_adapter: software" in source)

    def test_explicit_software_mode_requests_a_fallback_window_adapter(self):
        source = (ROOT / "src/crates/concat/src/gpu.rs").read_text(encoding="utf-8")
        acquire = section(source, "pub fn acquire()", "fn driver_of(")
        self.assertTrue('std::env::var("CONCAT_GPU")' in acquire)
        self.assertTrue("force_fallback_adapter: software" in acquire)


if __name__ == "__main__":
    unittest.main(verbosity=2)
