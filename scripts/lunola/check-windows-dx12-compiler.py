# SPDX-License-Identifier: AGPL-3.0-or-later
"""Source contract only; DX12 shader selection is not a GPU runtime test."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
POLICY = re.compile(
    r'backend_options\.dx12\.shader_compiler\s*=\s*'
    r'wgpu::Dx12Compiler::from_env\(\)\s*\.unwrap_or\(wgpu::Dx12Compiler::Fxc\)'
)


class WindowsDx12CompilerContract(unittest.TestCase):
    def test_window_and_compositor_pin_fxc_but_keep_the_explicit_wgpu_override(self):
        for relative in (
            'src/crates/concat/src/gpu.rs',
            'src/crates/concat-render/src/gpu.rs',
        ):
            with self.subTest(path=relative):
                source = (ROOT / relative).read_text(encoding='utf-8')
                self.assertIsNotNone(
                    POLICY.search(source),
                    f'{relative} must default to FXC and honor WGPU_DX12_COMPILER',
                )


if __name__ == '__main__':
    unittest.main(verbosity=2)
