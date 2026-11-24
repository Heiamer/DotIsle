"""Build content-addressed update packs from an already assembled runtime; no Cargo call."""
import argparse, hashlib, json, pathlib, re, zipfile

INVENTORY = 'Assets/RuntimeFiles.json'

def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def group(path):
    if path.startswith('Assets/Fonts/'):
        stem = re.sub(r'[^a-z0-9-]+', '-', pathlib.PurePosixPath(path).stem.lower())
        return f'font-{stem}-{hashlib.sha256(path.encode()).hexdigest()[:8]}'
    if path.startswith('Assets/Sounds/'): return 'sounds'
    if path.startswith('Assets/TransportIcons/'): return 'transport-icons'
    if path.startswith('Assets/'): return 'artwork'
    if path == 'Portable.json': return 'portable-mode'
    if '/' not in path and path.lower().endswith(('.dll', '.exe')): return 'binary-' + path.replace('.', '-').lower()
    return 'documentation'

def build(runtime, output, version):
    output.mkdir(parents=True, exist_ok=True)
    inventory = json.loads((runtime / INVENTORY).read_text(encoding='utf-8'))
    if (inventory['schemaVersion'], inventory['product'], inventory['version']) != (1, 'DotIsle', version):
        raise ValueError('Wrong runtime inventory version')
    groups = {}
    actual = {}
    for path in sorted(runtime.rglob('*')):
        if not path.is_file(): continue
        if path.is_symlink(): raise ValueError('Symlinks are not runtime assets')
        name = path.relative_to(runtime).as_posix()
        if name == INVENTORY: continue
        actual[name] = digest(path)
        groups.setdefault(group(name), []).append((name, path, actual[name]))
    if actual != inventory['files'] or not {'LyricIsland.exe', 'Portable.json'} <= actual.keys():
        raise ValueError('Runtime files do not match the full OTA inventory')
    packages = []
    for module, files in sorted(groups.items()):
        identity = hashlib.sha256(json.dumps([(n,h) for n,_,h in files], separators=(',',':')).encode()).hexdigest()
        archive = output / f'{module}-{identity}.zip'
        with zipfile.ZipFile(archive, 'w') as z:
            for name, path, _ in files:
                info = zipfile.ZipInfo('Module/' + name, (2020,1,1,0,0,0))
                z.writestr(info, path.read_bytes(), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
        packages.append(dict(id=module, archive=archive.name, sha256=digest(archive), bytes=archive.stat().st_size, files={n:h for n,_,h in files}))
    index = output / f'LyricIsland-{version}-Modules.json'
    index.write_text(json.dumps(dict(schemaVersion=2, version=version, runtimeVersion=version, packages=packages), ensure_ascii=False, indent=2), encoding='utf-8')
    return index

if __name__ == '__main__':
    p=argparse.ArgumentParser(); p.add_argument('--runtime',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('--version',required=True)
    a=p.parse_args();build(a.runtime,a.output,a.version)
