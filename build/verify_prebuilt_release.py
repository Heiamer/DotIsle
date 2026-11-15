"""Verify locally accepted release assets before publishing; never rebuild them."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import zipfile

REPORTS = (
    'source/report.json', 'settings/report.json', 'settings/layout-audit/report.json',
    'themes/report.json', 'themes/background-report.json', 'covers/report.json',
    'native-timers/report.json', 'liquid/report.json', 'glow/report.json',
    'modules/report.json', 'desktop/report.json',
)
CHECKS = ('rustWorkspace', 'goCenter', 'netEaseReader', 'modulePackages')


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def asset_name(name):
    return bool(name) and name not in ('.', '..') and not any(c in name for c in '/\\:')


def verify(directory, version, source_commit, manifest_sha256):
    require(re.fullmatch(r'\d+\.\d+\.\d+(?:-rc[1-9]\d*)?', version), 'Invalid version')
    require(re.fullmatch(r'[0-9a-f]{40}', source_commit), 'Expected a full source commit')
    require(re.fullmatch(r'[0-9a-f]{64}', manifest_sha256), 'Invalid manifest digest')
    prefix = f'DotIsle-{version}'
    manifest = directory / f'{prefix}-SHA256.txt'
    require(digest(manifest) == manifest_sha256, 'Checksum manifest changed after local verification')
    entries = {}
    for line in manifest.read_text(encoding='utf-8-sig').splitlines():
        match = re.fullmatch(r'([0-9a-fA-F]{64})\s+(.+)', line)
        require(match is not None, 'Invalid checksum entry')
        expected, name = match.groups()
        require(asset_name(name) and name not in entries and name != manifest.name, 'Unsafe or duplicate asset name')
        require((directory / name).is_file() and digest(directory / name) == expected.lower(), f'Asset checksum mismatch: {name}')
        entries[name] = expected.lower()
    required = {f'{prefix}-{suffix}' for suffix in ('Setup.exe', 'Portable.zip', 'Source.zip', 'Validation.zip', 'Notes.md', 'Verification.json')}
    required.update({f'LyricIsland-{version}-{suffix}' for suffix in ('Portable.zip', 'SHA256.txt', 'Modules.json')})
    require(required <= entries.keys(), f'Missing required assets: {sorted(required - entries.keys())}')
    require({p.name for p in directory.iterdir()} == set(entries) | {manifest.name}, 'Unexpected or unlisted release assets')
    evidence = json.loads((directory / f'{prefix}-Verification.json').read_text(encoding='utf-8-sig'))
    require(evidence.get('success') is True and evidence.get('version') == version, 'Local acceptance did not pass')
    require(evidence.get('sourceCommit') == source_commit, 'Local acceptance used a different source commit')
    require(all(evidence.get('checks', {}).get(key) is True for key in CHECKS), 'Local command checks are incomplete')
    with zipfile.ZipFile(directory / f'{prefix}-Validation.zip') as archive:
        for relative in REPORTS:
            matches = [name for name in archive.namelist() if name == relative or name.endswith('/' + relative)]
            require(len(matches) == 1, f'Missing or ambiguous acceptance report: {relative}')
            report = json.loads(archive.read(matches[0]).decode('utf-8-sig'))
            require(report.get('success') is True, f'Acceptance failed: {relative}')
    with zipfile.ZipFile(directory / f'{prefix}-Source.zip') as archive:
        require(archive.comment.decode('ascii') == source_commit, 'Source archive does not match the verified commit')
    with zipfile.ZipFile(directory / f'{prefix}-Portable.zip') as archive:
        runtime = {}
        for item in archive.infolist():
            parts = item.filename.split('/')
            if len(parts) == 2 and parts[0] == prefix and Path(parts[1]).suffix.lower() in ('.exe', '.dll'):
                require(parts[1] not in runtime, 'Duplicate runtime binary')
                with archive.open(item) as stream:
                    runtime[parts[1]] = hashlib.file_digest(stream, 'sha256').hexdigest()
        require(runtime and runtime == evidence.get('runtimeSha256'), 'Portable runtime differs from the locally accepted binaries')
        require(sum(name.startswith('dotisle_') and name.endswith('.dll') for name in runtime) == 15, 'Expected 15 service DLLs')
        require({'LyricIsland.exe', 'LyricIsland.Center.exe', 'LyricIsland.Settings.exe', 'LyricIsland.Updater.exe'} <= runtime.keys(), 'Missing a runtime executable')
    index = json.loads((directory / f'LyricIsland-{version}-Modules.json').read_text(encoding='utf-8-sig'))
    require(index.get('version') == version and index.get('runtimeVersion') == version, 'Module index version mismatch')
    require(index.get('packages'), 'Empty module index')
    for package in index['packages']:
        name = package.get('archive', '')
        require(asset_name(name) and entries.get(name) == package.get('sha256'), f'Module archive mismatch: {name}')
    return dict(success=True, version=version, sourceCommit=source_commit, assets=len(entries), acceptanceReports=len(REPORTS), runtimeBinaries=len(runtime))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--source-commit', required=True)
    parser.add_argument('--manifest-sha256', required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    report = verify(args.directory, args.version, args.source_commit, args.manifest_sha256)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report))


if __name__ == '__main__':
    main()
