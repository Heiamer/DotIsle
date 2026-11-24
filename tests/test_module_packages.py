import importlib.util, pathlib, tempfile, unittest, json, zipfile
spec=importlib.util.spec_from_file_location('packs',pathlib.Path(__file__).parents[1]/'build/package_modules.py')
packs=importlib.util.module_from_spec(spec);spec.loader.exec_module(packs)
class Packs(unittest.TestCase):
 def test_resource_change_preserves_binary_and_font_packages(self):
  with tempfile.TemporaryDirectory() as d:
   root=pathlib.Path(d);runtime=root/'runtime';(runtime/'Assets/Sounds').mkdir(parents=True);(runtime/'Assets/Fonts').mkdir()
   (runtime/'dotisle_core.dll').write_bytes(b'core');(runtime/'Assets/Fonts/font.ttf').write_bytes(b'font');sound=runtime/'Assets/Sounds/tone.wav';sound.write_bytes(b'old')
   (runtime/'LyricIsland.exe').write_bytes(b'app');(runtime/'Portable.json').write_bytes(b'portable')
   def build(output, version):
    manifest=runtime/packs.INVENTORY
    files={p.relative_to(runtime).as_posix():packs.digest(p) for p in runtime.rglob('*') if p.is_file() and p!=manifest}
    manifest.write_text(json.dumps(dict(schemaVersion=1,product='DotIsle',version=version,files=files)))
    return json.loads(packs.build(runtime,root/output,version).read_text())
   a=build('a','3.5.2');sound.write_bytes(b'new')
   b=build('b','3.5.3')
   aa={p['id']:p for p in a['packages']};bb={p['id']:p for p in b['packages']}
   font=packs.group('Assets/Fonts/font.ttf')
   self.assertEqual(aa[font],bb[font]);self.assertEqual(aa['binary-dotisle_core-dll'],bb['binary-dotisle_core-dll']);self.assertNotEqual(aa['sounds']['archive'],bb['sounds']['archive'])
   for p in b['packages']:
    archive=root/'b'/p['archive'];self.assertEqual(packs.digest(archive),p['sha256'])
    with zipfile.ZipFile(archive) as z:self.assertEqual(set(z.namelist()),{'Module/'+f for f in p['files']})
if __name__=='__main__':unittest.main()
