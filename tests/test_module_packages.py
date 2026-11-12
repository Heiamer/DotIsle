import importlib.util, pathlib, tempfile, unittest, json, zipfile
spec=importlib.util.spec_from_file_location('packs',pathlib.Path(__file__).parents[1]/'build/package_modules.py')
packs=importlib.util.module_from_spec(spec);spec.loader.exec_module(packs)
class Packs(unittest.TestCase):
 def test_resource_change_preserves_binary_and_font_packages(self):
  with tempfile.TemporaryDirectory() as d:
   root=pathlib.Path(d);runtime=root/'runtime';(runtime/'Assets/Sounds').mkdir(parents=True);(runtime/'Assets/Fonts').mkdir()
   (runtime/'dotisle_core.dll').write_bytes(b'core');(runtime/'Assets/Fonts/font.ttf').write_bytes(b'font');sound=runtime/'Assets/Sounds/tone.wav';sound.write_bytes(b'old')
   a=json.loads(packs.build(runtime,root/'a','3.5.2','3.5.1').read_text());sound.write_bytes(b'new')
   b=json.loads(packs.build(runtime,root/'b','3.5.3','3.5.1').read_text())
   aa={p['id']:p for p in a['packages']};bb={p['id']:p for p in b['packages']}
   self.assertEqual(aa['fonts'],bb['fonts']);self.assertEqual(aa['binary-dotisle_core-dll'],bb['binary-dotisle_core-dll']);self.assertNotEqual(aa['sounds']['archive'],bb['sounds']['archive'])
   for p in b['packages']:
    archive=root/'b'/p['archive'];self.assertEqual(packs.digest(archive),p['sha256'])
    with zipfile.ZipFile(archive) as z:self.assertEqual(set(z.namelist()),{'Module/'+f for f in p['files']})
if __name__=='__main__':unittest.main()
