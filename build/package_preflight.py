"""Fail fast when an input that build/package_release.py needs is missing or unusable.

Run it right before packaging: one readable Chinese checklist instead of a traceback in the
middle of the staging copy. Read-only apart from creating --output and the optional --json file.
"""
from pathlib import Path
import argparse
import importlib.util
import json
import re
import sys

# Keep in sync with build/package_release.py: binary map (lines 34-37), module DLL list
# (line 45), bundled docs (line 52) and the notes file it copies before staging (line 56).
SHIPPED_BINARIES = (
    'LyricIsland.exe',
    'lyric-updater.exe',
    'lyric-uninstall.exe',
    'lyric-setup.exe',
    'LyricIsland.Center.exe',
    'DotIsle-Allegro.exe',
)
MODULE_DLLS = (
    'dotisle_core.dll',
    'dotisle_desktop.dll',
    'dotisle_focus.dll',
    'dotisle_todos.dll',
    'dotisle_music.dll',
    'dotisle_lyrics.dll',
    'dotisle_notifications.dll',
    'dotisle_history.dll',
    'dotisle_automation.dll',
    'dotisle_devices.dll',
    'dotisle_osd.dll',
    'dotisle_sound.dll',
    'dotisle_widgets.dll',
    'dotisle_themes.dll',
    'dotisle_glow.dll',
)
RUNTIME_DOCS = ('README.md', 'GESTURE-TUTORIAL.md', 'NETEASE-SEEKFIX.md', 'CHANGELOG.md',
                'LICENSE', 'FONT-NOTICE.md', 'THIRD-PARTY-NOTICES.md', 'REWRITE.md', 'docs/UNIFIED-CONTROL-CENTER.md', 'docs/DISCLAIMER.md')

HINT_BINARIES = '在仓库根跑 tools/li-cargo.ps1 build --release --workspace --bins --lib --locked 重新产出 Rust 运行时'
HINT_CENTER = '先运行 Go/settings-center/build.cmd，再将构建出的 LyricIsland.Center.exe 放入 target/release。'
HINT_SHADER = '把 Assets/ShaderCache/Hybrid29 整个目录放回仓库，或把 --shader-cache 指向真实存在的着色器缓存目录'
HINT_ISCC = '安装 Inno Setup 6，或把 ISCC.exe 的绝对路径传给 --iscc（默认 C:\\Program Files (x86)\\Inno Setup 6\\ISCC.exe）'
HINT_ASSETS = '补齐仓库内被复制的静态资源（Assets/、licenses/、docs/brand/、build/e_sqlite3.dll、build/installer.iss、build/ChineseSimplified.isl）'
HINT_DOCS = '把缺失的根目录文档放回仓库根（package_release.py 逐个 copy2 到运行时目录）'
HINT_RESOURCES = '确认 build/pe_resources.py 完整，且当前 Python 能导入它（embed_icon / update_resources 必须存在）'
HINT_NOTES = '在 release-notes/ 下补 DotIsle-<版本>-Notes.md（verify_release_source.py 同样校验它）'
HINT_NOTES_OUTPUT = '先把 release-notes/DotIsle-<版本>-Notes.md 复制到 --output 目录：package_release.py 第 56 行在打包前就要读到它（CI 打包步骤已自动复制）'
HINT_FRESH = '删掉已存在的 --output/DotIsle-<版本> 目录，或换一个新的 --version'
HINT_OUTPUT = '换一个可写的 --output 目录（当前目录无法写入）'
HINT_VERSION = '让 --version 与 Rust/Cargo.toml 里 workspace.package.version 一致（CI 里由 Resolve release version 步骤统一提供）'


def echo(text=''):
    print(text)


def main():
    try:
        sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    except Exception:
        pass
    parser = argparse.ArgumentParser(description='Check every input build/package_release.py needs.')
    parser.add_argument('--binary-dir', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--shader-cache', type=Path, required=True)
    parser.add_argument('--iscc', type=Path, required=True)
    parser.add_argument('--json', type=Path)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    binary_dir = args.binary_dir.resolve()
    output = args.output.resolve()
    shader = args.shader_cache.resolve()
    rows = []

    def add(label, ok, info, hint):
        rows.append({'item': label, 'ok': ok, 'detail': info, 'hint': '' if ok else hint})

    def file_item(label, path, hint):
        if not path.exists():
            add(label, False, '不存在: %s' % path, hint)
        elif not path.is_file():
            add(label, False, '不是文件: %s' % path, hint)
        elif path.stat().st_size <= 0:
            add(label, False, '文件为空: %s' % path, hint)
        else:
            add(label, True, '%d 字节' % path.stat().st_size, hint)

    def dir_item(label, path, hint):
        if not path.is_dir():
            add(label, False, '目录不存在: %s' % path, hint)
            return False
        files = [p for p in path.rglob('*') if p.is_file()]
        add(label, True, '%d 个文件' % len(files), hint)
        return True

    echo('DotIsle 打包前置校验（对齐 build/package_release.py 的输入）')
    echo('  版本:        %s' % args.version)
    echo('  仓库根:      %s' % root)
    echo('  二进制目录:  %s' % binary_dir)
    echo('  输出目录:    %s' % output)
    echo('  着色器缓存:  %s' % shader)
    echo('  ISCC:        %s' % args.iscc)
    echo()

    # Release version shape mirrors build/verify_release_source.py; names derive from it.
    add('版本号格式（X.Y.Z 或 X.Y.Z-rcN）', bool(re.fullmatch(r'\d+\.\d+\.\d+(?:-rc[1-9]\d*)?', args.version)),
        args.version, '按 X.Y.Z 或 X.Y.Z-rcN 传入 --version')
    manifest = root / 'Rust/Cargo.toml'
    if manifest.is_file():
        found = re.search(r'^version = "([^"]+)"', manifest.read_text(encoding='utf-8'), re.M)
        workspace_version = found.group(1) if found else ''
        add('版本号与 Rust/Cargo.toml 一致', workspace_version == args.version,
            ('workspace.package.version=%s' % workspace_version) if workspace_version else '未读到 version 字段', HINT_VERSION)
    else:
        add('版本号与 Rust/Cargo.toml 一致', False, '缺少 Rust/Cargo.toml', HINT_VERSION)

    if binary_dir.is_dir():
        add('--binary-dir 存在', True, str(binary_dir), '')
    else:
        add('--binary-dir 存在', False, '目录不存在: %s' % binary_dir, HINT_BINARIES)
    for name in SHIPPED_BINARIES:
        file_item('--binary-dir/%s' % name, binary_dir / name, HINT_CENTER if name == 'LyricIsland.Center.exe' else HINT_BINARIES)
    for name in MODULE_DLLS:
        file_item('--binary-dir/%s' % name, binary_dir / name, HINT_BINARIES)

    if shader.is_dir():
        cached = [p for p in shader.rglob('*') if p.is_file()]
        add('--shader-cache 目录存在且非空', bool(cached), '%d 个文件' % len(cached), HINT_SHADER)
    else:
        add('--shader-cache 目录存在且非空', False, '目录不存在: %s' % shader, HINT_SHADER)

    dir_item('仓库 Assets/', root / 'Assets', HINT_ASSETS)
    dir_item('仓库 licenses/', root / 'licenses', HINT_ASSETS)
    file_item('仓库 Assets/LyricIsland-v3.ico（embed_icon 输入）', root / 'Assets/LyricIsland-v3.ico', HINT_ASSETS)
    dir_item('仓库 docs/brand/', root / 'docs/brand', HINT_ASSETS)
    file_item('仓库 build/e_sqlite3.dll', root / 'build/e_sqlite3.dll', HINT_ASSETS)
    file_item('仓库 build/installer.iss', root / 'build/installer.iss', HINT_ASSETS)
    file_item('仓库 build/ChineseSimplified.isl（installer.iss 引用）', root / 'build/ChineseSimplified.isl', HINT_ASSETS)
    for doc in RUNTIME_DOCS:
        file_item('仓库 %s' % doc, root / doc, HINT_DOCS)

    # pe_resources embeds the icon and the install payload, so it must import on this interpreter.
    # Bytecode writing stays off so a check never leaves build/__pycache__ behind.
    previous_bytecode = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        spec = importlib.util.spec_from_file_location('dotisle_pe_resources', root / 'build/pe_resources.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        missing_api = [name for name in ('embed_icon', 'update_resources') if not callable(getattr(module, name, None))]
        add('build/pe_resources.py 可导入', not missing_api,
            '缺少 %s' % '、'.join(missing_api) if missing_api else 'embed_icon / update_resources 就绪', HINT_RESOURCES)
    except Exception as error:
        add('build/pe_resources.py 可导入', False, '%s: %s' % (type(error).__name__, error), HINT_RESOURCES)
    finally:
        sys.dont_write_bytecode = previous_bytecode

    if args.iscc.exists() and args.iscc.is_file():
        add('--iscc 可执行文件存在', True, '%d 字节' % args.iscc.stat().st_size, '')
    else:
        add('--iscc 可执行文件存在', False, '不存在: %s' % args.iscc, HINT_ISCC)

    notes_repo = root / 'release-notes' / ('DotIsle-%s-Notes.md' % args.version)
    file_item('release-notes/DotIsle-<版本>-Notes.md', notes_repo, HINT_NOTES)
    try:
        output.mkdir(parents=True, exist_ok=True)
        probe = output / '.preflight-write-probe'
        probe.write_text('ok', encoding='ascii')
        probe.unlink()
        add('--output 目录可写', True, str(output), '')
    except Exception as error:
        add('--output 目录可写', False, '%s: %s' % (type(error).__name__, error), HINT_OUTPUT)
    notes_output = output / ('DotIsle-%s-Notes.md' % args.version)
    file_item('--output/DotIsle-<版本>-Notes.md（打包前必须存在）', notes_output, HINT_NOTES_OUTPUT)
    stale = output / ('DotIsle-%s' % args.version)
    add('--output/DotIsle-<版本> 尚未存在', not stale.exists(),
        '已存在，package_release.py 会拒绝复用' if stale.exists() else '空闲', HINT_FRESH)

    for row in rows:
        echo('%-9s %s%s' % ('[OK]' if row['ok'] else '[MISSING]', row['item'],
                            '' if row['ok'] else '  —— ' + row['detail']))
    failed = [row for row in rows if not row['ok']]
    echo()
    echo('合计 %d 项：OK %d，MISSING %d' % (len(rows), len(rows) - len(failed), len(failed)))
    echo('MISSING=%d' % len(failed))
    if failed:
        grouped = {}
        for row in failed:
            grouped.setdefault(row['hint'], []).append(row['item'])
        echo()
        echo('补救建议:')
        for index, (hint, items) in enumerate(grouped.items(), 1):
            echo('  %d. %s' % (index, hint))
            echo('     影响 %d 项：%s' % (len(items), '、'.join(items)))
    else:
        echo('全部输入就绪，可以执行 python build/package_release.py')

    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps({'version': args.version, 'success': not failed,
                                         'missing': len(failed), 'checks': rows},
                                        ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    return 1 if failed else 0


if __name__ == '__main__':
    raise SystemExit(main())
