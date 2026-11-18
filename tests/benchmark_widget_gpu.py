"""Isolate adapter selection and swapchain latency in the real widget host."""
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
from verify_unified_center import children, stop, wait, widget_windows


kernel = c.WinDLL('kernel32', use_last_error=True)
kernel.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
kernel.OpenProcess.restype = w.HANDLE
kernel.CloseHandle.argtypes = [w.HANDLE]


def run(runtime, scratch, profile, seconds, music_source=None, require_idle_exit=False):
    app = Runtime(runtime, scratch / uuid.uuid4().hex)
    handle = None
    try:
        app.data.mkdir(parents=True)
        app.write_settings()
        settings = app.data / 'settings.json'
        state = json.loads(settings.read_text(encoding='utf-8'))
        state.update(StartupSignatureEnabled=False, StartupGreetingEnabled=False,
                     WeatherAlertsEnabled=False, AllowedSources=[music_source or 'LyricIsland.Verify13'])
        settings.write_text(json.dumps(state), encoding='utf-8')
        env = {'DOTISLE_TRACE_GPU': '1'}
        if profile:
            env['DOTISLE_TEST_WIDGET_GPU'] = profile
        app.launch(env=env)
        pid = lambda: next((p['pid'] for p in children(app) if p['name'] == 'LyricIsland.Center.exe'), None)

        def add(kind, count, x):
            reply = app.request('widgetAdd', {'kind': kind, 'size': 'medium', 'style': 'original', 'x': x, 'y': 160})
            assert reply['success'], reply
            wait(lambda: pid() and len(widget_windows(pid())) == count, f'{count} desktop widgets', seconds=15)
            return reply['id']

        def measure(count):
            time.sleep(3)
            assert not center_window(owner)
            start = time.monotonic()
            cpu, _, _ = sample(SimpleNamespace(_handle=handle))
            rows = []
            for second in range(1, seconds + 1):
                time.sleep(max(0, start + second - time.monotonic()))
                rows.append(sample(SimpleNamespace(_handle=handle)))
            elapsed = time.monotonic() - start
            assert pid() == owner and len(widget_windows(owner)) == count
            state = app.request('state')
            assert not state['desktopWidgetError'], state['desktopWidgetError']
            return dict(privateBytes=statistics.median(r[1] for r in rows),
                        workingSetBytes=statistics.median(r[2] for r in rows),
                        peakSamplePrivateBytes=max(r[1] for r in rows),
                        cpuMsPerSecond=(rows[-1][0] - cpu) / elapsed, samples=rows)

        started = time.monotonic()
        note = add('note', 1, 160)
        ready = time.monotonic() - started
        owner = pid()
        handle = kernel.OpenProcess(0x410, False, owner)
        assert handle, c.WinError(c.get_last_error())
        results = dict(startupMs=ready * 1000, note=measure(1))
        focus = add('focus', 2, 550)
        clock = add('worldClock', 3, 940)
        results['threeWidgets'] = measure(3)
        ids = [clock, focus, note]
        if music_source:
            wait(lambda: app.request('state')['isPlaying'], 'playing media session', seconds=12)
            ids.insert(0, add('music', 4, 1330))
            first = app.request('state')
            results['playingMusic'] = measure(4)
            last = app.request('state')
            assert first['isPlaying'] and last['isPlaying'] and last['position'] > first['position']
        for widget in ids:
            assert app.request('widgetRemove', {'id': widget})['success']
        wait(lambda: not widget_windows(owner), 'last widget window is destroyed', seconds=10)
        if require_idle_exit:
            wait(lambda: not pid(), 'last widget removal releases host', seconds=10)
        results['log'] = str(app.data / 'process.log')
        return results
    finally:
        if handle:
            kernel.CloseHandle(handle)
        try:
            stop(app)
        finally:
            app.stop()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('runtime', type=Path)
    parser.add_argument('--baseline', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--seconds', type=int, default=5)
    parser.add_argument('--runs', type=int, default=2)
    parser.add_argument('--music-source')
    parser.add_argument('--require-idle-exit', action='store_true')
    parser.add_argument('--profiles', nargs='+', choices=['default', 'high-3', 'high-2', 'low-3', 'low-2', 'low-2-select'],
                        default=['high-3', 'low-2', 'high-2', 'low-3'])
    args = parser.parse_args()
    if args.seconds < 1 or args.runs < 1:
        parser.error('Specify positive seconds and runs')
    args.report.parent.mkdir(parents=True, exist_ok=True)
    profiles = [(p, args.runtime, None if p == 'default' else p) for p in args.profiles]
    if args.baseline:
        profiles.insert(0, ('previous-package', args.baseline, None))
    output = dict(seconds=args.seconds, runs=args.runs, workloads={})
    for trial in range(args.runs):
        for label, path, profile in (profiles if trial % 2 == 0 else profiles[::-1]):
            result = run(path, args.report.parent / 'widget-gpu-data', profile, args.seconds,
                         args.music_source, args.require_idle_exit and label != 'previous-package')
            output['workloads'].setdefault(label, []).append(result)
            args.report.write_text(json.dumps(output, indent=2) + '\n', encoding='utf-8')
            print(label, {k: round(v['privateBytes'] / 2**20, 1) for k, v in result.items()
                          if isinstance(v, dict)}, flush=True)
