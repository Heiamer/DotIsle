"""Measure a packaged island and control center in one isolated Windows session."""
import argparse
import ctypes as c
from ctypes import wintypes as w
import json
import statistics
import time
import uuid
from pathlib import Path
from types import SimpleNamespace

from benchmark_center import center_window
from benchmark_idle_reminders import sample
from verify_pomodoro import Runtime
from verify_unified_center import children, stop, user, wait

MIB = 1024 ** 2
PAGES = ('overview', 'plugins', 'island', 'about', 'widgets', 'timers', 'about', 'overview')
kernel = c.WinDLL('kernel32', use_last_error=True)
kernel.OpenProcess.restype = w.HANDLE
kernel.CloseHandle.argtypes = [w.HANDLE]
user.PostMessageW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]


def measure(process, seconds):
    start = time.monotonic()
    cpu, _, _ = sample(process)
    rows = []
    while time.monotonic() - start < seconds:
        rows.append(sample(process))
        time.sleep(.1)
    return dict(workingMiB=round(statistics.median(r[2] for r in rows) / MIB, 2),
                privateMiB=round(statistics.median(r[1] for r in rows) / MIB, 2),
                maxSampleWorkingMiB=round(max(r[2] for r in rows) / MIB, 2),
                maxSamplePrivateMiB=round(max(r[1] for r in rows) / MIB, 2),
                cpuMsPerSecond=round((rows[-1][0] - cpu) / (time.monotonic() - start), 2))


def verify(runtime, scratch, captures):
    app = Runtime(runtime, scratch / uuid.uuid4().hex)
    handle = None
    results = {'center': {}}
    try:
        app.data.mkdir(parents=True)
        app.write_settings()
        config = app.data / 'settings.json'
        settings = json.loads(config.read_text(encoding='utf-8'))
        settings.update(StartupSignatureEnabled=False, StartupGreetingEnabled=False,
                        IslandKeepVisibleEnabled=False, WeatherAlertsEnabled=False,
                        AllowedSources=['LyricIsland.Verify13'])
        config.write_text(json.dumps(settings), encoding='utf-8')
        version = json.loads((app.runtime / 'Assets/RuntimeFiles.json').read_text(encoding='utf-8'))['version']
        (app.data / 'center-release-notes.json').write_text(json.dumps({'version': version}), encoding='utf-8')
        app.launch()
        results['hidden'] = measure(app.process, 2)

        def update(key, value):
            assert app.request('update', {'key': key, 'value': value})['success']

        update('IslandResidentShowTime', True)
        update('IslandKeepVisibleEnabled', True)
        wait(lambda: app.request('state')['residentIdle'], 'resident island')
        results['resident'] = measure(app.process, 4)
        assert app.request('testPointerClick')['success']
        wait(lambda: app.request('state')['expanded'], 'expanded island')
        results['expanded'] = measure(app.process, 4)
        if captures:
            assert app.request('debugSnapshot', str(app.data / 'island.png'))['success']

        geometry = app.request('state')['islandGeometry']['card']
        close = {'x': (geometry['w'] - 34) / geometry['w'], 'y': 28 / geometry['h']}
        assert app.request('testPointerClick', close)['success']
        wait(lambda: not app.request('state')['expanded'], 'island collapsed')
        update('IslandKeepVisibleEnabled', False)
        wait(lambda: not app.request('state')['islandVisible'], 'hidden island')
        time.sleep(10)
        results['hiddenAfter10Seconds'] = measure(app.process, 2)
        state = app.request('state')
        results['rendererAfter10Seconds'] = state['rendererLoaded']

        def center_child():
            return next((p['pid'] for p in children(app) if p['name'] == 'LyricIsland.Center.exe'), None)

        for i, page in enumerate(PAGES):
            assert app.request('settings', {'page': page})['success']
            pid = wait(center_child, 'center child')
            hwnd = wait(lambda: center_window(pid), 'center window')
            wait(lambda: not app.request('state')['pendingPage'], f'{page} acknowledged')
            if handle is None:
                handle = kernel.OpenProcess(0x410, False, pid)
                assert handle, c.WinError(c.get_last_error())
            results['center'][f'{i}-{page}'] = measure(SimpleNamespace(_handle=handle), 3)
        results['dpi'] = user.GetDpiForWindow(w.HWND(hwnd))
        # Native WM_CLOSE works even when the Settings overlay covers the titlebar.
        assert user.PostMessageW(w.HWND(hwnd), 0x10, 0, 0), c.WinError(c.get_last_error())
        wait(lambda: not center_window(pid), 'center hidden', seconds=4)
        wait(lambda: not center_child(), 'center exited with no widgets or edits', seconds=10)
        results['closedCenterExited'] = True
        results['captureDirectory'] = str(app.data) if captures else None
        return results
    finally:
        if handle:
            kernel.CloseHandle(handle)
        stop(app)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--captures', action='store_true')
    parser.add_argument('--enforce', action='store_true', help='require the optimized budgets (island about 100 MiB; center under 200 MiB)')
    args = parser.parse_args()
    args.report.parent.mkdir(parents=True, exist_ok=True)
    report = verify(args.runtime.resolve(), args.report.resolve().parent / 'budget-data', args.captures)
    island_phases = [report[p] for p in ('resident', 'expanded')]
    report['meetsBudget'] = (max(p['workingMiB'] for p in island_phases) <= 110
                             and max(p['maxSampleWorkingMiB'] for p in island_phases) <= 120
                             and max(p['maxSamplePrivateMiB'] for p in report['center'].values()) <= 200
                             and not report['rendererAfter10Seconds'] and report['closedCenterExited'])
    args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False), flush=True)
    if args.enforce and not report['meetsBudget']:
        raise SystemExit('Memory budget exceeded or renderer did not retire')
