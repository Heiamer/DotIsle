import json
from pathlib import Path
import sys
import tempfile
import unittest
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'build'))
from prepare_public_release import digest, prepare
from package_modules import INVENTORY, build


class PublicReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / 'accepted'
        self.source.mkdir()
        self.output = self.root / 'public'
        self.version = '5.0.0-rc1'
        for suffix in ('Setup.exe', 'Portable.7z', 'Source.zip', 'Validation.zip', 'Verification.json'):
            (self.source / f'DotIsle-{self.version}-{suffix}').write_bytes(suffix.encode() * (10 if suffix == 'Setup.exe' else 1))
        (self.source / f'DotIsle-{self.version}-Notes.md').write_text('# 点屿\n\n- 修复歌词。\n', encoding='utf-8')
        self.zip_name = f'LyricIsland-{self.version}-Portable.zip'
        runtime = self.root / 'runtime'
        (runtime / 'Assets/Fonts').mkdir(parents=True)
        files = {'LyricIsland.exe': b'app', 'Portable.json': b'{}', 'Assets/Fonts/test.ttf': b'font'}
        for name, contents in files.items():
            (runtime / name).write_bytes(contents)
        (runtime / INVENTORY).write_text(json.dumps(dict(schemaVersion=1, product='DotIsle', version=self.version,
            files={name: digest(runtime / name) for name in files})))
        with zipfile.ZipFile(self.source / self.zip_name, 'w') as archive:
            for name in [*files, INVENTORY]:
                archive.write(runtime / name, f'LyricIsland-{self.version}/{name}')
        self.index = build(runtime, self.source, self.version)
        self.legacy_name = f'LyricIsland-{self.version}-SHA256.txt'
        (self.source / self.legacy_name).write_text(f'{digest(self.source / self.zip_name)}  {self.zip_name}\n')
        (self.source / 'binary-core.zip').write_bytes(b'internal')
        self.manifest = self.source / f'DotIsle-{self.version}-SHA256.txt'
        self.manifest.write_text(''.join(f'{digest(p)}  {p.name}\n' for p in sorted(self.source.iterdir()) if p != self.manifest))

    def test_only_public_downloads_ship_and_legacy_ota_uses_pinned_full_bundle(self):
        before = {p.name: digest(p) for p in self.source.iterdir()}
        mirror = self.root / 'gitee'
        report = prepare(self.source, self.output, self.version, mirror)
        self.assertIn(self.index.name, report['remove'])
        self.assertFalse(report['componentOta'])
        self.assertEqual(len(report['keep']), 7)
        for package in json.loads(self.index.read_text())['packages']:
            self.assertIn(package['archive'], report['remove'])
        self.assertIn('binary-core.zip', report['remove'])
        self.assertEqual({p.name for p in self.output.iterdir()}, set(report['keep']))
        full_line = f'{digest(self.source / self.zip_name)}  {self.zip_name}\n'
        self.assertEqual((self.output / self.legacy_name).read_text(), full_line)
        self.assertEqual((mirror / self.legacy_name).read_text(), full_line)
        self.assertEqual(set(report['giteeKeep']), {p.name for p in mirror.iterdir()})
        self.assertEqual(digest(mirror / self.zip_name), digest(self.output / self.zip_name))
        for line in (self.output / self.manifest.name).read_text().splitlines():
            expected, name = line.split('  ')
            self.assertEqual(digest(self.output / name), expected)
        self.assertEqual({p.name: digest(p) for p in self.source.iterdir()}, before)

    def test_changed_public_asset_is_not_published(self):
        (self.source / self.zip_name).write_bytes(b'unverified replacement')
        with self.assertRaisesRegex(ValueError, 'Unverified public asset'):
            prepare(self.source, self.output, self.version)
        self.assertFalse(self.output.exists())

    def test_oversized_portable_is_not_published(self):
        portable = self.source / f'DotIsle-{self.version}-Portable.7z'
        portable.write_bytes(b'X' * 1024)
        self.manifest.write_text(''.join(f'{digest(p)}  {p.name}\n' for p in sorted(self.source.iterdir()) if p != self.manifest))
        with self.assertRaisesRegex(ValueError, 'smaller than the installer'):
            prepare(self.source, self.output, self.version)

    def test_existing_output_is_not_overwritten(self):
        self.output.mkdir()
        with self.assertRaisesRegex(ValueError, 'new public asset directory'):
            prepare(self.source, self.output, self.version)

    def test_public_staging_does_not_require_component_assets(self):
        for package in json.loads(self.index.read_text())['packages']:
            (self.source / package['archive']).unlink()
        self.index.unlink()
        report = prepare(self.source, self.output, self.version)
        self.assertEqual(len(report['keep']), 7)


if __name__ == '__main__':
    unittest.main()
