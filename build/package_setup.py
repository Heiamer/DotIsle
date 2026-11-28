"""Compile the 5.2.9 setup exe from an existing portable runtime directory."""
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

os.environ['PATH'] = r'C:\Windows\System32' + os.pathsep + os.environ.get('PATH', '')

from package_release import archive, sha, embed_icon, update_resources

root = Path(__file__).resolve().parents[1]
runtime = Path(sys.argv[1])
version = sys.argv[2]
binary_dir = Path(sys.argv[3])
out = Path(sys.argv[4])
iscc = Path(sys.argv[5])

scratch = Path(tempfile.mkdtemp(prefix='dotisle-setup-'))
engine = scratch / 'DotIsle.Engine.exe'
shutil.copy2(binary_dir / 'lyric-setup.exe', engine)
embed_icon(engine, root / 'Assets/LyricIsland-v3.ico')
payload = scratch / 'InstallPayload.zip'
archive(runtime, payload, 'LyricIsland-' + version)
update_resources(engine, [
    (10, 'DotIsle.InstallPayload.zip', payload.read_bytes()),
    (10, 'DotIsle.InstallPayload.sha256', sha(payload).encode('ascii')),
])
out.mkdir(parents=True, exist_ok=True)
subprocess.run([
    str(iscc), '/Qp', '/DAppVersion=' + version,
    '/DStageDir=' + str(runtime.resolve()),
    '/DOutputDir=' + str(out.resolve()),
    '/DEngineFile=' + str(engine),
    str(root / 'build/installer.iss'),
], check=True)
print(out / (f'DotIsle-{version}-Setup.exe'))