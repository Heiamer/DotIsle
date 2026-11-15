"""Publication must reject modified assets, failed acceptance and mismatched source."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'build'))
from verify_prebuilt_release import CHECKS, REPORTS, digest, verify


class PrebuiltReleaseTests(unittest.TestCase):
    version = '5.0.0-rc1'
    commit = 'a' * 40

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.prefix = 'DotIsle-' + self.version
        self.runtime = {f'dotisle_module{i}.dll': b'module' for i in range(14)}
        self.runtime['dotisle_desktop.dll'] = b'desktop'
        self.runtime.update({name: b'binary' for name in ('LyricIsland.exe', 'LyricIsland.Center.exe', 'LyricIsland.Updater.exe')})
        self.portable()
        with zipfile.ZipFile(self.root / f'{self.prefix}-Source.zip', 'w') as archive:
            archive.comment = self.commit.encode('ascii')
            archive.writestr('source/README.md', 'source')
        self.reports()
        (self.root / f'{self.prefix}-Setup.exe').write_bytes(b'setup')
        (self.root / f'{self.prefix}-Notes.md').write_text('notes')
        (self.root / f'LyricIsland-{self.version}-Portable.zip').write_bytes(b'legacy')
        (self.root / f'LyricIsland-{self.version}-SHA256.txt').write_text('legacy checksum')
        (self.root / 'module.zip').write_bytes(b'module update')
        index = dict(version=self.version, runtimeVersion=self.version, packages=[dict(archive='module.zip', sha256=digest(self.root / 'module.zip'))])
        (self.root / f'LyricIsland-{self.version}-Modules.json').write_text(json.dumps(index))
        evidence = dict(success=True, version=self.version, sourceCommit=self.commit, checks=dict.fromkeys(CHECKS, True), runtimeSha256={name: hashlib.sha256(data).hexdigest() for name, data in self.runtime.items()})
        (self.root / f'{self.prefix}-Verification.json').write_text(json.dumps(evidence))

    def portable(self):
        with zipfile.ZipFile(self.root / f'{self.prefix}-Portable.zip', 'w') as archive:
            for name, data in self.runtime.items():
                archive.writestr(self.prefix + '/' + name, data)

    def reports(self, failed=None):
        with zipfile.ZipFile(self.root / f'{self.prefix}-Validation.zip', 'w') as archive:
            for name in REPORTS:
                archive.writestr('validation/' + name, json.dumps(dict(success=name != failed)))

    def manifest(self):
        path = self.root / f'{self.prefix}-SHA256.txt'
        path.write_text(''.join(f'{digest(item)}  {item.name}\n' for item in sorted(self.root.iterdir()) if item != path))
        return digest(path)

    def check(self):
        return verify(self.root, self.version, self.commit, self.manifest())

    def test_complete_local_acceptance_can_publish(self):
        self.assertTrue(self.check()['success'])

    def test_changed_asset_is_rejected(self):
        expected = self.manifest()
        (self.root / f'{self.prefix}-Setup.exe').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
            verify(self.root, self.version, self.commit, expected)

    def test_failed_report_is_rejected(self):
        self.reports('center/control-center/report.json')
        with self.assertRaisesRegex(ValueError, 'Acceptance failed'):
            self.check()

    def test_source_commit_must_match(self):
        with zipfile.ZipFile(self.root / f'{self.prefix}-Source.zip', 'a') as archive:
            archive.comment = ('b' * 40).encode('ascii')
        with self.assertRaisesRegex(ValueError, 'Source archive'):
            self.check()

    def test_runtime_must_match_accepted_binaries(self):
        self.runtime['LyricIsland.Center.exe'] = b'different runtime'
        self.portable()
        with self.assertRaisesRegex(ValueError, 'Portable runtime differs'):
            self.check()

    def test_manifest_cannot_change_after_dispatch(self):
        expected = self.manifest()
        (self.root / f'{self.prefix}-SHA256.txt').write_text('replacement')
        with self.assertRaisesRegex(ValueError, 'Checksum manifest changed'):
            verify(self.root, self.version, self.commit, expected)


if __name__ == '__main__':
    unittest.main()
