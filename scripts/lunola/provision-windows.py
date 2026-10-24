# SPDX-License-Identifier: AGPL-3.0-or-later
"""Fetch and verify upstream-pinned Windows x64 development dependencies.

Stores files in a caller-specified external tools directory. No global install
or PATH edits. Does not substitute dependencies when a pinned URL fails.
"""
from __future__ import annotations
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import shutil
import tarfile
import urllib.request
import zipfile

NAMES = ['ffmpeg-win64', 'onnxruntime-win-x64', 'sherpa-onnx-win-x64-shared-MT-Release-lib']


def provision(root: Path) -> None:
    repo = Path(__file__).resolve().parents[2]
    pins = {}
    for line in (repo / 'scripts/ci/deps.lock').read_text().splitlines():
        if line and not line.startswith('#'):
            name, url, digest = line.split()
            pins[name] = (url, digest)
    root.mkdir(parents=True, exist_ok=True)

    def fetch(name: str) -> dict:
        url, expected = pins[name]
        dest = root / url.rsplit('/', 1)[1]
        print(f'Fetching {name}', flush=True)
        if not dest.exists():
            temp = dest.with_suffix(dest.suffix + '.partial')
            req = urllib.request.Request(url, headers={'User-Agent': 'Lunola-Mova-build-provisioner'})
            with urllib.request.urlopen(req, timeout=180) as response, temp.open('wb') as out:
                shutil.copyfileobj(response, out, 1024 * 1024)
            temp.replace(dest)
        with dest.open('rb') as stream:
            actual = hashlib.file_digest(stream, 'sha256').hexdigest()
        if actual != expected:
            raise RuntimeError(f'{name}: SHA-256 mismatch: {actual} != {expected}')
        target = root / name
        marker = target / '.verified-sha256'
        if not marker.exists() or marker.read_text().strip() != expected:
            target.mkdir(exist_ok=True)
            if zipfile.is_zipfile(dest):
                with zipfile.ZipFile(dest) as archive:
                    archive.extractall(target)
            else:
                with tarfile.open(dest) as archive:
                    archive.extractall(target, filter='data')
            marker.write_text(expected + '\n')
        print(f'Verified {name}: {dest.stat().st_size} bytes', flush=True)
        return {'name': name, 'url': url, 'sha256': actual, 'bytes': dest.stat().st_size,
                'archive': str(dest), 'extracted': str(target)}

    with ThreadPoolExecutor(max_workers=3) as pool:
        records = list(pool.map(fetch, NAMES))
    (root / 'dependency-manifest.json').write_text(json.dumps(records, indent=2) + '\n')
    print('Dependency provisioning completed; inspect required libraries before build.', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--root', type=Path, required=True)
    provision(parser.parse_args().root)
