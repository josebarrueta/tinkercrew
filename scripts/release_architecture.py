#!/usr/bin/env python3
"""Check the current Apple-silicon-only release policy; not signature approval."""
import argparse
from pathlib import Path
import plistlib
import subprocess
import sys


MACHO_MAGIC = {bytes.fromhex(value) for value in (
    'feedface', 'cefaedfe', 'feedfacf', 'cffaedfe',
    'cafebabe', 'bebafeca', 'cafebabf', 'bfbafeca',
)}


def check_app(app):
    contents = app / 'Contents'
    with (contents / 'Info.plist').open('rb') as source:
        info = plistlib.loads(source.read(1024 * 1024))
    name = info.get('CFBundleExecutable')
    if not isinstance(name, str) or not name or name in {'.', '..'} or '/' in name or '\\' in name:
        raise ValueError('Invalid main executable name.')
    main = contents / 'MacOS' / name
    if main.is_symlink() or not main.is_file():
        raise ValueError('Missing regular main executable.')
    found_main = False
    for binary in contents.rglob('*'):
        if binary.is_symlink() or not binary.is_file():
            continue
        with binary.open('rb') as source:
            magic = source.read(4)
        if magic not in MACHO_MAGIC:
            continue
        result = subprocess.run(['lipo', '-archs', str(binary)], capture_output=True, text=True, check=True, timeout=30)
        if result.stdout.split() != ['arm64']:
            raise ValueError('Release code must be arm64 only; Intel and Universal releases are deferred.')
        found_main |= binary == main
    if not found_main:
        raise ValueError('Main executable is not Mach-O arm64 code.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('app', type=Path)
    args = parser.parse_args()
    try:
        check_app(args.app)
    except ValueError as error:
        print(str(error), file=sys.stderr)
        return 1
    except (OSError, plistlib.InvalidFileException, subprocess.SubprocessError):
        print('Could not verify release architecture.', file=sys.stderr)
        return 1
    print('Apple-silicon-only architecture check passed (arm64); signing and acceptance remain separate.')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
