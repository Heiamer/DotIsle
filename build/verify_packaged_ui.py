"""Run native acceptance with bounded waits and visible capture progress."""
from pathlib import Path
import argparse
import json
import os
import subprocess
import time
import uuid


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    runtime = args.runtime.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, DOTISLE_TRACE_GPU='1', LYRICISLAND_TEST_PIPE='LyricIsland.Settings.test.ci.' + uuid.uuid4().hex)
    checks = [('settings', '--verify', 2700), ('themes', '--theme-capture', 900), ('covers', '--cover-capture', 600)]
    for name, flag, timeout in checks:
        destination = output / name
        destination.mkdir(exist_ok=True)
        started = time.monotonic()
        print(f'[{name}] Starting native acceptance; timeout {timeout}s', flush=True)
        with (destination / 'process.log').open('wb') as log:
            process = subprocess.Popen([str(runtime / 'LyricIsland.Settings.exe'), flag, str(destination)],
                cwd=runtime, env=environment, stdout=log, stderr=log,
                creationflags=subprocess.CREATE_NO_WINDOW)
            try:
                while True:
                    try:
                        code = process.wait(timeout=30)
                        break
                    except subprocess.TimeoutExpired:
                        captures = list(destination.rglob('*.png'))
                        latest = max(captures, key=lambda p: p.stat().st_mtime).relative_to(destination) if captures else 'initializing'
                        elapsed = round(time.monotonic() - started)
                        print(f'[{name}] {elapsed}s; {len(captures)} captures; latest: {latest}', flush=True)
                        if elapsed >= timeout:
                            raise TimeoutError(f'{name} exceeded {timeout}s; last capture: {latest}')
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=15)
        if code != 0:
            raise RuntimeError(f'{name} exited with {code}; see {destination / "process.log"}')
        report = json.loads((destination / 'report.json').read_text(encoding='utf-8-sig'))
        if report.get('success') is not True:
            raise RuntimeError(f'{name} acceptance failed: {report}')
        print(f'[{name}] Passed in {time.monotonic() - started:.1f}s', flush=True)


if __name__ == '__main__':
    main()
