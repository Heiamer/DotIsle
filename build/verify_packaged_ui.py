"""Verify the shipped Go executable, its embedded desktop DLL and native state flow."""
from pathlib import Path
import argparse
import json
import os
import subprocess
import sys
import uuid

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument('--runtime',type=Path,required=True)
    ap.add_argument('--output',type=Path,required=True)
    args=ap.parse_args();runtime=args.runtime.resolve();output=args.output.resolve()
    output.mkdir(parents=True,exist_ok=True)
    if any((runtime/name).exists() for name in ('LyricIsland.Settings.exe','dotisle_settings.dll')):
        raise RuntimeError('Legacy settings must not ship in the unified runtime')
    environment=dict(os.environ, LYRICISLAND_TEST_PIPE='LyricIsland.Settings.test.ci.'+uuid.uuid4().hex)
    for name in ('DOTISLE_MODULE_DIR','DOTISLE_DESKTOP_DLL'):
        environment.pop(name,None)
    destination=output/'center';destination.mkdir(exist_ok=True)
    with (destination/'process.log').open('wb') as log:
        subprocess.run([str(runtime/'LyricIsland.Center.exe'),'--verify',str(destination)],cwd=runtime,env=environment,
            stdout=log,stderr=log,timeout=1200,check=True,creationflags=subprocess.CREATE_NO_WINDOW)
    for relative in ('report.json','control-center/report.json','desktop/report.json'):
        report=json.loads((destination/relative).read_text(encoding='utf-8-sig'))
        if report.get('success') is not True: raise RuntimeError(f'UI acceptance failed: {relative}: {report}')
    print('Packaged Go views and Rust desktop renderer passed',flush=True)
    root=Path(__file__).resolve().parents[1]
    subprocess.run([sys.executable,str(root/'tests/verify_unified_center.py'),'--runtime',str(runtime),'--scratch',str(output/'desktop-runtime'),'--report',str(output/'desktop/report.json')],
        env=environment,timeout=180,check=True,creationflags=subprocess.CREATE_NO_WINDOW)

if __name__=='__main__': main()
