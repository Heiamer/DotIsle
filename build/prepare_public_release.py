"""Separate user downloads from local acceptance and component-pack artifacts."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil

from portable_archive import require_smaller


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def prepare(directory, output, version, gitee_output=None):
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-rc[1-9]\d*)?', version):
        raise ValueError('Invalid version')
    notes_name = f'DotIsle-{version}-Notes.md'
    public = [f'DotIsle-{version}-{suffix}' for suffix in ('Setup.exe', 'Portable.7z', 'Source.zip', 'Notes.md')]
    public += [f'LyricIsland-{version}-{suffix}' for suffix in ('Portable.zip', 'SHA256.txt')]
    manifest_name = f'DotIsle-{version}-SHA256.txt'
    entries = {}
    for line in (directory / manifest_name).read_text(encoding='utf-8-sig').splitlines():
        match = re.fullmatch(r'([0-9a-fA-F]{64})\s+([^/\\:]+)', line)
        if not match or match[2] in entries or match[2] in ('.', '..'):
            raise ValueError('Invalid checksum manifest')
        entries[match[2]] = match[1].lower()
    for name in public:
        if entries.get(name) != digest(directory / name):
            raise ValueError('Unverified public asset: ' + name)
    notes = (directory / notes_name).read_text(encoding='utf-8-sig').strip()
    if not notes or len(notes) > 6000 or not any(line.startswith('- ') and line[2:].strip() for line in notes.splitlines()):
        raise ValueError('Release notes must include a non-empty change list readable by old updaters')
    require_smaller(directory / public[1], directory / public[0])
    if output.exists() or (gitee_output is not None and (gitee_output.exists() or gitee_output.resolve() == output.resolve())):
        raise ValueError('Use a new public asset directory')
    legacy_checksum = f'LyricIsland-{version}-SHA256.txt'
    legacy_zip = f'LyricIsland-{version}-Portable.zip'
    output.mkdir(parents=True)
    for name in public:
        shutil.copy2(directory / name, output / name)
    full_line = f'{entries[legacy_zip]}  {legacy_zip}\n'
    (output / legacy_checksum).write_text(full_line, encoding='ascii')
    if gitee_output is not None:
        gitee_output.mkdir(parents=True)
        shutil.copy2(output / legacy_zip, gitee_output / legacy_zip)
        (gitee_output / legacy_checksum).write_text(full_line, encoding='ascii')
    (output / manifest_name).write_text(''.join(f'{digest(output / name)}  {name}\n' for name in sorted(public)), encoding='ascii')
    keep = sorted(public + [manifest_name])
    return dict(success=True, version=version, keep=keep,
                remove=sorted(p.name for p in directory.iterdir() if p.is_file() and p.name not in keep),
                fullBundleOta=True, componentOta=False, giteeKeep=[legacy_checksum, legacy_zip],
                manifestSha256=digest(output / manifest_name))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--gitee-output', type=Path)
    args = parser.parse_args()
    result = prepare(args.directory, args.output, args.version, args.gitee_output)
    args.plan.parent.mkdir(parents=True, exist_ok=True)
    args.plan.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(result))
