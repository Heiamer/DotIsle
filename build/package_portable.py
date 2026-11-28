"""Build only the portable DotIsle distribution (runtime dir + 7z + legacy zip).

Mirrors the runtime assembly of package_release.py but skips Inno Setup and the
installer comparison, so a portable package can be produced for manual checks.
"""
from pathlib import Path
import argparse, hashlib, json, os, shutil, struct, subprocess

# Prefer Windows' libarchive bsdtar over Git's GNU tar (no 7zip writer).
os.environ['PATH'] = os.pathsep.join([r'C:\Windows\System32', os.environ.get('PATH', '')])

from package_release import copy_runtime_assets, archive, write_runtime_inventory
from pe_resources import embed_icon
from portable_archive import create_portable, hashes, verify_portable

MODULES = ['dotisle_core.dll', 'dotisle_desktop.dll', 'dotisle_focus.dll', 'dotisle_todos.dll',
           'dotisle_music.dll', 'dotisle_lyrics.dll', 'dotisle_notifications.dll',
           'dotisle_history.dll', 'dotisle_automation.dll', 'dotisle_devices.dll',
           'dotisle_osd.dll', 'dotisle_sound.dll', 'dotisle_widgets.dll',
           'dotisle_themes.dll', 'dotisle_glow.dll']


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def check_center(binary_dir):
    center = binary_dir / 'LyricIsland.Center.exe'
    if not center.is_file() or center.stat().st_size == 0:
        raise RuntimeError('Build the Go control center before packaging')
    pe = center.read_bytes()
    offset = struct.unpack_from('<I', pe, 0x3c)[0]
    if struct.unpack_from('<H', pe, offset + 24 + 68)[0] != 2:
        raise RuntimeError('Center must use the Windows GUI subsystem (-H=windowsgui)')
    renderer = subprocess.run([str(center.resolve()), '--check-renderer-build'],
                              capture_output=True, timeout=15, creationflags=subprocess.CREATE_NO_WINDOW)
    if renderer.returncode != 0 or renderer.stdout.strip() != b'DOTISLE_HARDWARE_RENDERER=1':
        raise RuntimeError('Center is missing the production MyGo renderer patch; run Go/settings-center/build.cmd')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--binary-dir', type=Path, required=True)
    ap.add_argument('--output', type=Path, required=True)
    ap.add_argument('--work-dir', type=Path, required=True)
    ap.add_argument('--version', required=True)
    ap.add_argument('--shader-cache', type=Path, required=True)
    ap.add_argument('--release-notes', type=Path, required=True)
    args = ap.parse_args()

    root = Path(__file__).resolve().parents[1]
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    check_center(args.binary_dir)

    from verify_lyrics_engine import verify as verify_lyrics_engine
    verify_lyrics_engine(args.binary_dir)

    name = 'DotIsle-' + args.version
    runtime = out / name
    if runtime.exists():
        raise RuntimeError(f'Runtime directory already exists: {runtime}')
    runtime.mkdir()

    for built, shipped in {'LyricIsland.exe': 'LyricIsland.exe',
                           'lyric-updater.exe': 'LyricIsland.Updater.exe',
                           'lyric-uninstall.exe': 'Uninstall.exe'}.items():
        target = runtime / shipped
        shutil.copy2(args.binary_dir / built, target)
        embed_icon(target, root / 'Assets/LyricIsland-v3.ico')
    shutil.copy2(args.binary_dir / 'LyricIsland.Center.exe', runtime / 'LyricIsland.Center.exe')
    for module in MODULES:
        shutil.copy2(args.binary_dir / module, runtime / module)

    copy_runtime_assets(root / 'Assets', runtime / 'Assets')
    shutil.copytree(args.shader_cache, runtime / 'Assets/ShaderCache/Hybrid29')
    shutil.copytree(root / 'licenses', runtime / 'licenses')
    shutil.copy2(root / 'build/e_sqlite3.dll', runtime / 'e_sqlite3.dll')
    for doc in ['README.md', 'GESTURE-TUTORIAL.md', 'NETEASE-SEEKFIX.md', 'CHANGELOG.md',
                'LICENSE', 'FONT-NOTICE.md', 'THIRD-PARTY-NOTICES.md', 'REWRITE.md']:
        shutil.copy2(root / doc, runtime / doc)
    shutil.copytree(root / 'docs/brand', runtime / 'docs/brand')
    for doc in ['PLUGIN-DEVELOPMENT.md', 'UNIFIED-CONTROL-CENTER.md', 'DISCLAIMER.md', 'AI-QUOTA-NATIVE.md']:
        shutil.copy2(root / 'docs' / doc, runtime / 'docs' / doc)
    shutil.copytree(root / 'examples/plugins', runtime / 'examples/plugins')
    shutil.copy2(args.release_notes, runtime / 'ReleaseNotes.md')

    (runtime / 'Portable.json').write_text(
        json.dumps({'portable': True, 'version': args.version}) + '\n', encoding='utf8')
    write_runtime_inventory(runtime, args.version)

    portable = out / (name + '-Portable.7z')
    create_portable(runtime, portable)
    verify_portable(portable, name, hashes(runtime))

    legacy = 'LyricIsland-' + args.version
    legacy_zip = out / (legacy + '-Portable.zip')
    archive(runtime, legacy_zip, legacy)
    (out / (legacy + '-SHA256.txt')).write_text(
        sha(legacy_zip) + '  ' + legacy_zip.name + '\n', encoding='ascii')
    print(json.dumps({'runtime': str(runtime), 'portable7z': str(portable),
                      'portableZip': str(legacy_zip),
                      'portable7zSize': portable.stat().st_size,
                      'portableZipSize': legacy_zip.stat().st_size},
                     ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
