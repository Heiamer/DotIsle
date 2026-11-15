import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

spec=importlib.util.spec_from_file_location('transfer',Path(__file__).resolve().parents[1]/'build/restore_release_transfer.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)


class TransferTests(unittest.TestCase):
    def bundle(self,root,name='accepted.bin',expected=None):
        body=b'locally accepted bytes'
        plan={'schemaVersion':1,'operations':[{'name':name,'kind':'full','payload':'body.bin','sha256':expected or hashlib.sha256(body).hexdigest()}]}
        path=root/'transport.zip'
        with zipfile.ZipFile(path,'w') as archive:
            archive.writestr('plan.json',json.dumps(plan));archive.writestr('body.bin',body)
        return path,module.digest(path)

    def test_restores_exact_bytes_and_removes_transport(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);path,digest=self.bundle(root)
            module.restore(root,path,digest,root/'bases',fetch=False)
            self.assertEqual((root/'accepted.bin').read_bytes(),b'locally accepted bytes')
            self.assertFalse(path.exists())

    def test_rejects_tampered_transport(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);path,digest=self.bundle(root);path.write_bytes(path.read_bytes()+b'changed')
            with self.assertRaises(ValueError):module.restore(root,path,digest,root/'bases',fetch=False)
            self.assertFalse((root/'accepted.bin').exists())

    def test_rejects_unsafe_name_and_wrong_restored_hash(self):
        for name,expected in (('../outside.bin',None),('accepted.bin','0'*64)):
            with self.subTest(name=name),tempfile.TemporaryDirectory() as temporary:
                root=Path(temporary);path,digest=self.bundle(root,name,expected)
                with self.assertRaises(ValueError):module.restore(root,path,digest,root/'bases',fetch=False)
                self.assertFalse((root/'accepted.bin').exists())


if __name__=='__main__':unittest.main()
