"""Build content-addressed update packs from an already assembled runtime; no Cargo call."""
import argparse, hashlib, json, pathlib, zipfile

def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def group(path):
    if path.startswith('Assets/Fonts/'): return 'fonts'
    if path.startswith('Assets/Sounds/'): return 'sounds'
    if path.startswith('Assets/TransportIcons/'): return 'transport-icons'
    if path.startswith('Assets/'): return 'artwork'
    return 'binary-' + path.replace('.', '-').lower()

def build(runtime, output, version, runtime_version):
    output.mkdir(parents=True, exist_ok=True)
    groups = {}
    for path in sorted(runtime.rglob('*')):
        if not path.is_file(): continue
        if path.is_symlink(): raise ValueError('Symlinks are not runtime assets')
        name = path.relative_to(runtime).as_posix()
        if name == 'Installed.json' or name == 'ModuleState.json': continue
        if not (name.startswith('Assets/') or '/' not in name and path.suffix.lower() in ('.exe', '.dll')): continue
        groups.setdefault(group(name), []).append((name, path, digest(path)))
    packages = []
    for module, files in sorted(groups.items()):
        identity = hashlib.sha256(json.dumps([(n,h) for n,_,h in files], separators=(',',':')).encode()).hexdigest()
        archive = output / f'{module}-{identity}.zip'
        with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as z:
            for name, path, _ in files:
                info = zipfile.ZipInfo('Module/' + name, (2020,1,1,0,0,0))
                info.compress_type = zipfile.ZIP_DEFLATED
                z.writestr(info, path.read_bytes())
        packages.append(dict(id=module, archive=archive.name, sha256=digest(archive), files={n:h for n,_,h in files}))
    index = output / f'LyricIsland-{version}-Modules.json'
    index.write_text(json.dumps(dict(schemaVersion=1, version=version, runtimeVersion=runtime_version, packages=packages), ensure_ascii=False, indent=2), encoding='utf-8')
    return index

if __name__ == '__main__':
    p=argparse.ArgumentParser(); p.add_argument('--runtime',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('--version',required=True);p.add_argument('--runtime-version')
    a=p.parse_args();build(a.runtime,a.output,a.version,a.runtime_version or a.version)
