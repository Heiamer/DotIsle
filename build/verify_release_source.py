"""Check release metadata and the declared source/license inventory before packaging."""
from pathlib import Path
import argparse
import json
import re
import tomllib


def verify(root, version):
    errors = []
    workspace = tomllib.loads((root / 'Rust/Cargo.toml').read_text(encoding='utf-8'))
    package = workspace['workspace']['package']
    if package['version'] != version:
        errors.append('Workspace version differs from the requested release version')
    if package['authors'] != ['Heiamer'] or package['license'] != 'MIT':
        errors.append('Unexpected project author or license metadata')
    copyright_lines = re.findall(r'^Copyright .*$', (root / 'LICENSE').read_text(encoding='utf-8'), re.M)
    if copyright_lines != ['Copyright (c) 2026 Heiamer']:
        errors.append('Root MIT copyright does not match the project metadata')
    lock = tomllib.loads((root / 'Rust/Cargo.lock').read_text(encoding='utf-8'))
    locked = {p['name']: p for p in lock['package'] if 'source' not in p}
    for member in workspace['workspace']['members']:
        name = tomllib.loads((root / 'Rust' / member / 'Cargo.toml').read_text(encoding='utf-8'))['package']['name']
        if locked.get(name, {}).get('version') != version:
            errors.append(f'Workspace lock version differs: {name}')
    installer = (root / 'build/installer.iss').read_text(encoding='utf-8')
    if f'#define AppVersion "{version}"' not in installer or 'AppPublisher=Heiamer' not in installer:
        errors.append('Installer author or default version differs')
    manifest = json.loads((root / 'Assets/UpdateManifest.json').read_text(encoding='utf-8'))
    if '-rc' in version:
        stable_tag = manifest.get('tag_name', '')
        stable_version = stable_tag.removeprefix('v')
        stable = re.fullmatch(r'\d+\.\d+\.\d+', stable_version)
        if (not stable or manifest.get('published') is not True or
                manifest.get('draft') is not False or manifest.get('prerelease') is not False):
            errors.append('RC builds must preserve a published stable update manifest')
        elif tuple(map(int, stable_version.split('.'))) > tuple(map(int, version.split('-')[0].split('.'))):
            errors.append('Stable update manifest is newer than the RC base version')
    elif manifest['tag_name'] != f'v{version}':
        errors.append('Update manifest version differs')
    if not (root / f'release-notes/DotIsle-{version}-Notes.md').is_file():
        errors.append('Release notes are missing')
    policy = json.loads((root / 'build/source-policy.json').read_text(encoding='utf-8'))
    for removed in policy['removedCoreModules']:
        if (root / removed).exists():
            errors.append(f'Retired module is still present: {removed}')
    license_files = sorted(p.relative_to(root / 'licenses').as_posix()
                           for p in (root / 'licenses').rglob('*') if p.is_file())
    if license_files != policy['licenseFiles']:
        errors.append('License inventory differs from the reviewed source policy')
    for required in ['licenses/qq-lyric-codec-MIT.txt', 'licenses/NeteaseCloudMusicApi-MIT.txt',
                     'licenses/roxmltree-MIT.txt', 'licenses/Rewrite-Rust-Licenses.txt',
                     'licenses/iced-x86-MIT.txt', 'licenses/lazy_static-MIT.txt',
                     'Rust/vendor/qq-lyric-codec/LICENSE']:
        if not (root / required).is_file():
            errors.append(f'Required dependency license is missing: {required}')
    return {'success': not errors, 'version': version, 'rootCopyright': copyright_lines,
            'workspacePackages': len(workspace['workspace']['members']),
            'removedModulesChecked': len(policy['removedCoreModules']),
            'licenseFiles': len(license_files), 'errors': errors}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--version', required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-rc[1-9]\d*)?', args.version):
        parser.error('Use a three-part version, optionally followed by -rcN (N >= 1)')
    result = verify(Path(__file__).resolve().parents[1], args.version)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(result, ensure_ascii=False))
    raise SystemExit(0 if result['success'] else 1)


if __name__ == '__main__':
    main()
