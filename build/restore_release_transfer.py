"""Restore accepted release bytes from an optional, hash-pinned transport bundle."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import zipfile


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def checked_name(name):
    if not isinstance(name, str) or not name or name in ('.', '..') or any(c in name for c in '/\\:'):
        raise ValueError('Invalid transfer filename')
    return name


def checked_digest(value):
    if not isinstance(value, str) or not re.fullmatch('[0-9a-f]{64}', value):
        raise ValueError('Invalid transfer digest')
    return value


def restore(directory, transfer, expected, bases, zstd='zstd', fetch=True):
    if digest(transfer) != checked_digest(expected):
        raise ValueError('Transfer bundle changed')
    bases.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(transfer) as archive, tempfile.TemporaryDirectory(dir=bases) as temporary:
        work=Path(temporary)
        plan=json.loads(archive.read('plan.json'))
        if plan.get('schemaVersion') != 1:
            raise ValueError('Unsupported transfer schema')
        seen=set()
        for operation in plan['operations']:
            name=checked_name(operation['name']); expected_file=checked_digest(operation['sha256'])
            if name in seen:
                raise ValueError('Duplicate transfer target')
            seen.add(name); target=directory/name
            if target.is_file() and digest(target)==expected_file:
                continue
            payload=checked_name(operation['payload'])
            if operation['kind']=='full':
                candidate=work/'result.bin';candidate.write_bytes(archive.read(payload))
            elif operation['kind']=='zstd':
                reference=work/'reference.bin'
                with reference.open('wb') as combined:
                    for part in operation['bases']:
                        part_name=checked_name(part['name']);part_hash=checked_digest(part['sha256'])
                        if part['kind']=='release':
                            # Only this repository's published RC1 is a transfer dictionary.
                            if part.get('tag')!='v5.0.0-rc1':
                                raise ValueError('Unsupported base release')
                            path=bases/part_name
                            if not path.exists() and fetch:
                                subprocess.run(['gh','release','download','v5.0.0-rc1','--repo','Heiamer/DotIsle','--pattern',part_name,'--dir',str(bases)],check=True)
                        elif part['kind']=='restored':
                            path=directory/part_name
                        else:
                            raise ValueError('Unsupported base type')
                        if not path.is_file() or digest(path)!=part_hash:
                            raise ValueError('Transfer dictionary mismatch: '+part_name)
                        with path.open('rb') as source:
                            shutil.copyfileobj(source,combined)
                patch=work/'patch.zst';patch.write_bytes(archive.read(payload))
                candidate=work/'result.bin'
                subprocess.run([zstd,'-q','-f','-d','--memory=2048MB','--patch-from='+str(reference),str(patch),'-o',str(candidate)],check=True)
            else:
                raise ValueError('Unsupported transfer operation')
            if digest(candidate)!=expected_file:
                raise ValueError('Restored file differs from local acceptance: '+name)
            shutil.move(candidate,target)
    transfer.unlink()
    return {'success':True,'restoredFiles':len(seen)}


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    for name in ('directory','transfer','bases'):
        parser.add_argument('--'+name,type=Path,required=True)
    parser.add_argument('--sha256',required=True)
    args=parser.parse_args()
    print(json.dumps(restore(args.directory,args.transfer,args.sha256,args.bases)))
