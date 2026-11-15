import hashlib
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'build'))
from prepare_public_release import digest, prepare


class PublicReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / 'accepted'
        self.source.mkdir()
        self.output = self.root / 'public'
        self.version = '5.0.0-rc1'
        for suffix in ('Setup.exe', 'Portable.zip', 'Source.zip', 'Validation.zip', 'Verification.json'):
            (self.source / f'DotIsle-{self.version}-{suffix}').write_bytes(suffix.encode())
        self.zip_name = f'LyricIsland-{self.version}-Portable.zip'
        (self.source / self.zip_name).write_bytes(b'accepted runtime')
        self.legacy_name = f'LyricIsland-{self.version}-SHA256.txt'
        (self.source / self.legacy_name).write_text(f'{digest(self.source / self.zip_name)}  {self.zip_name}\n' + 'a' * 64 + '  LyricIsland-5.0.0-rc1-Modules.json\n')
        (self.source / 'binary-core.zip').write_bytes(b'internal')
        self.manifest = self.source / f'DotIsle-{self.version}-SHA256.txt'
        self.manifest.write_text(''.join(f'{digest(p)}  {p.name}\n' for p in sorted(self.source.iterdir()) if p != self.manifest))

    def test_only_public_downloads_ship_and_legacy_ota_uses_pinned_full_bundle(self):
        before = {p.name: digest(p) for p in self.source.iterdir()}
        report = prepare(self.source, self.output, self.version)
        self.assertEqual(len(report['keep']), 6)
        self.assertIn('binary-core.zip', report['remove'])
        self.assertEqual({p.name for p in self.output.iterdir()}, set(report['keep']))
        self.assertEqual((self.output / self.legacy_name).read_text(), f'{digest(self.source / self.zip_name)}  {self.zip_name}\n')
        self.assertNotIn('Modules.json', (self.output / self.legacy_name).read_text())
        for line in (self.output / self.manifest.name).read_text().splitlines():
            expected, name = line.split('  ')
            self.assertEqual(digest(self.output / name), expected)
        self.assertEqual({p.name: digest(p) for p in self.source.iterdir()}, before)

    def test_changed_public_asset_is_not_published(self):
        (self.source / self.zip_name).write_bytes(b'unverified replacement')
        with self.assertRaisesRegex(ValueError, 'Unverified public asset'):
            prepare(self.source, self.output, self.version)
        self.assertFalse(self.output.exists())

    def test_existing_output_is_not_overwritten(self):
        self.output.mkdir()
        with self.assertRaisesRegex(ValueError, 'new public asset directory'):
            prepare(self.source, self.output, self.version)


if __name__ == '__main__':
    unittest.main()
