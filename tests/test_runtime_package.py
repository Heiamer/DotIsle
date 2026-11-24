import json
from pathlib import Path
import sys
import tempfile
import unittest
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'build'))
from package_release import archive, copy_runtime_assets, sha, write_runtime_inventory


class RuntimePackageTests(unittest.TestCase):
    def test_omit_archival_assets_but_keep_fonts_and_verify_ota_files(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            source = root / 'source'
            old = ['.gitattributes', 'LyricIsland-v2.ico', 'LyricIsland-v2.png', 'ShaderCache/old.bin']
            old.extend(f'Fonts/HarmonyOS_Sans_{locale}{weight}.ttf'
                       for locale in ('', 'SC_') for weight in ('Regular', 'Medium', 'Bold'))
            kept = ['DemoCover.png', 'LyricIsland-v3.ico', 'LyricIsland-v3.png',
                    'Fonts/NotoSansSC.ttf', 'Fonts/NotoSansJP.ttf', 'Fonts/NotoSerifSC.ttf',
                    'Fonts/GoogleSansFlex-Regular.ttf', 'Fonts/HarmonyOS-Sans-LICENSE.txt',
                    'Fonts/FusionPixel12.ttf']
            for name in [*old, *kept]:
                file = source / name
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(name.encode())
            runtime = root / 'runtime'
            copy_runtime_assets(source, runtime / 'Assets')
            for name in old:
                self.assertTrue((source / name).is_file())
                self.assertFalse((runtime / 'Assets' / name).exists())
            shader = runtime / 'Assets/ShaderCache/Hybrid29/core.bin'
            shader.parent.mkdir(parents=True)
            shader.write_bytes(b'current shader cache')
            write_runtime_inventory(runtime, '5.0.7')
            manifest = json.loads((runtime / 'Assets/RuntimeFiles.json').read_text(encoding='utf-8'))
            self.assertEqual(manifest['files'], {f'Assets/{name}': sha(runtime / 'Assets' / name)
                                                for name in [*kept, 'ShaderCache/Hybrid29/core.bin']})
            portable = root / 'ota.zip'
            archive(runtime, portable, 'LyricIsland-5.0.7')
            with zipfile.ZipFile(portable) as z:
                self.assertIsNone(z.testzip())
                self.assertTrue(all(item.compress_type == zipfile.ZIP_DEFLATED for item in z.infolist()))
                self.assertEqual(set(z.namelist()),
                                 {f'LyricIsland-5.0.7/{name}' for name in [*manifest['files'], 'Assets/RuntimeFiles.json']})


if __name__ == '__main__':
    unittest.main()
