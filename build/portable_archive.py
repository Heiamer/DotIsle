"""Build and verify solid 7z user downloads, independent of the OTA ZIP."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
from tempfile import TemporaryDirectory


def tar_command():
    # Linux GNU tar cannot read 7z; Windows includes libarchive's tar.exe.
    binary = shutil.which('tar.exe' if os.name == 'nt' else 'bsdtar')
    if not binary:
        raise RuntimeError('libarchive bsdtar is required to package or verify 7z')
    return binary


def hashes(directory):
    result = {}
    for path in directory.rglob('*'):
        if path.is_symlink():
            raise ValueError('Portable runtime must not contain symlinks')
        if path.is_file():
            with path.open('rb') as stream:
                result[path.relative_to(directory).as_posix()] = hashlib.file_digest(stream, 'sha256').hexdigest()
    return result


def create_portable(runtime, archive):
    subprocess.run([tar_command(), '--format', '7zip', '--options',
                    'compression=lzma2,compression-level=9', '-cf', str(archive.resolve()),
                    '-C', str(runtime.parent.resolve()), runtime.name], check=True)


def verify_portable(archive, prefix, expected):
    tar = tar_command()
    archive = archive.resolve()
    listing = subprocess.check_output([tar, '-tf', str(archive)], encoding='utf-8').splitlines()
    allowed = {f'{prefix}/{name}' for name in expected}
    directories = {str(parent).replace(os.sep, '/') + '/' for name in allowed for parent in Path(name).parents if parent != Path('.')}
    if len(listing) != len(set(listing)) or set(listing) - allowed - directories or not allowed <= set(listing):
        raise ValueError('Unexpected entries in portable 7z')
    with TemporaryDirectory(prefix='dotisle-portable-', dir=archive.parent) as temp:
        subprocess.run([tar, '-xf', str(archive), '-C', temp], check=True)
        unpacked = Path(temp)
        if {p.name for p in unpacked.iterdir()} != {prefix} or hashes(unpacked / prefix) != expected:
            raise ValueError('Portable 7z differs from the verified runtime')


def require_smaller(portable, setup):
    if not 0 < portable.stat().st_size < setup.stat().st_size:
        raise ValueError('Portable 7z must be smaller than the installer')
