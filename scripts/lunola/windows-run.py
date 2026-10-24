# SPDX-License-Identifier: AGPL-3.0-or-later
"""Run Cargo or an executable in an isolated Windows native build environment.

The tools directory defaults to the Hermes local tools folder. The runner
uses installed MSVC/SDK and checksum-verified native dependency directories.
It never mutates system/user PATH or silently substitutes a pinned archive.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def one_directory(root: Path) -> Path:
    children = [p for p in root.iterdir() if p.is_dir()]
    if len(children) != 1:
        raise RuntimeError(f'Expected one extracted archive root in {root}, got {children}')
    return children[0]


def environment(tools: Path) -> dict[str, str]:
    vswhere = Path(os.environ.get('ProgramFiles(x86)', 'C:/Program Files (x86)')) / 'Microsoft Visual Studio/Installer/vswhere.exe'
    vs = subprocess.check_output([str(vswhere), '-latest', '-products', '*', '-requires',
                                 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property', 'installationPath'], text=True).strip()
    if not vs:
        raise RuntimeError('MSVC C++ workload not found')
    vcvars = Path(vs) / 'VC/Auxiliary/Build/vcvars64.bat'
    result = subprocess.run(f'cmd.exe /d /s /c ""{vcvars}" >nul && set"', capture_output=True, text=True, check=True)
    env = dict(os.environ)
    # Windows environment keys are case insensitive: normalise to avoid PATH/Path collisions.
    env = {k.upper(): v for k, v in env.items()}
    for line in result.stdout.splitlines():
        if '=' in line and not line.startswith('='):
            k, v = line.split('=', 1)
            env[k.upper()] = v
    native = tools / 'mova-native'
    ffmpeg = one_directory(native / 'ffmpeg-win64')
    ort = one_directory(native / 'onnxruntime-win-x64')
    sherpa = one_directory(native / 'sherpa-onnx-win-x64-shared-MT-Release-lib')
    libs, dlls = native / 'sherpa-libs', native / 'sherpa-dlls'
    libs.mkdir(exist_ok=True)
    dlls.mkdir(exist_ok=True)
    for pattern, dest in [('sherpa-onnx-*.lib', libs), ('sherpa-onnx-*.dll', dlls)]:
        files = list((sherpa / 'lib').glob(pattern))
        if not files:
            raise RuntimeError(f'Missing {pattern} in {sherpa}')
        for f in files:
            if not (dest / f.name).exists():
                shutil.copy2(f, dest / f.name)
    clang = tools / 'mova-libclang/clang/native'
    required = [ffmpeg / 'include/libavcodec/avcodec.h', ort / 'lib/onnxruntime.lib', clang / 'libclang.dll']
    for f in required:
        if not f.is_file():
            raise RuntimeError(f'Missing required dependency: {f}')
    env.update(RUSTUP_HOME=str(tools / 'mova-rustup'), CARGO_HOME=str(tools / 'mova-cargo'),
               FFMPEG_DIR=str(ffmpeg), LIBCLANG_PATH=str(clang), ORT_LIB_LOCATION=str(ort / 'lib'),
               ORT_DIR=str(ort), ORT_PREFER_DYNAMIC_LINK='1', SHERPA_ONNX_LIB_DIR=str(libs),
               GGML_NATIVE='OFF', GGML_AVX2='OFF', GGML_FMA='OFF', GGML_F16C='OFF', GGML_BMI2='OFF',
               CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_QUICK_DEBUG='0')
    cmake = Path(vs) / 'Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin'
    ninja = Path(vs) / 'Common7/IDE/CommonExtensions/Microsoft/CMake/Ninja'
    env['PATH'] = ';'.join(map(str, [tools / 'mova-cargo/bin', tools / 'mova-nextest', ffmpeg / 'bin', ort / 'lib', dlls, cmake, ninja])) + ';' + env['PATH']
    return env


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--repo', type=Path, required=True, help='repository root; Cargo cwd is its src directory')
    parser.add_argument('--tools', type=Path, default=Path(os.environ['LOCALAPPDATA']) / 'hermes/tools')
    parser.add_argument('--log', type=Path, required=True)
    parser.add_argument('--target-dir', type=Path)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    env = environment(args.tools)
    if args.target_dir:
        env['CARGO_TARGET_DIR'] = str(args.target_dir)
    command = args.command
    if command and command[0] == '--':
        command = command[1:]
    if not command:
        parser.error('command required after --')
    executable = shutil.which(command[0], path=env['PATH'])
    if executable:
        command[0] = executable
    args.log.parent.mkdir(parents=True, exist_ok=True)
    facts = {k: env[k] for k in ['RUSTUP_HOME', 'CARGO_HOME', 'FFMPEG_DIR', 'LIBCLANG_PATH',
                                'ORT_LIB_LOCATION', 'SHERPA_ONNX_LIB_DIR', 'VCTOOLSVERSION', 'WINDOWSSDKDIR']}
    print(json.dumps({'cwd': str(args.repo / 'src'), 'command': command, 'environment': facts}, indent=2), flush=True)
    with args.log.open('w', encoding='utf-8') as log:
        log.write(json.dumps({'cwd': str(args.repo / 'src'), 'command': command, 'environment': facts}, indent=2) + '\n')
        with subprocess.Popen(command, cwd=args.repo / 'src', env=env, stdout=subprocess.PIPE,
                              stderr=subprocess.STDOUT, text=True, encoding='utf-8', errors='replace', bufsize=1) as proc:
            for line in proc.stdout:
                print(line, end='', flush=True)
                log.write(line)
                log.flush()
            code = proc.wait()
        log.write(f'\nEXIT_CODE={code}\n')
    print(f'EXIT_CODE={code}', flush=True)
    sys.exit(code)
