"""Measure the visible native control center and its owning island separately."""
import argparse
import ctypes as c
import json
import statistics
import time
import uuid
from ctypes import wintypes as w
from pathlib import Path
from types import SimpleNamespace

from benchmark_idle_reminders import sample
from verify_pomodoro import Runtime
from verify_unified_center import children, wait, user, widget_windows, WindowCallback


def center_window(pid):
    found = []
    @WindowCallback
    def collect(hwnd, _):
        owner = w.DWORD()
        user.GetWindowThreadProcessId(hwnd, c.byref(owner))
        title = c.create_unicode_buffer(200)
        user.GetWindowTextW(hwnd, title, len(title))
        if owner.value == pid and user.IsWindowVisible(hwnd) and '控制中心' in title.value:
            found.append(hwnd)
        return True
    user.EnumWindows(collect, 0)
    return found[0] if found else None


def hide_center(window):
    """Click the test window's own caption button, without moving the cursor."""
    user.FindWindowExW.argtypes = [w.HWND, w.HWND, w.LPCWSTR, w.LPCWSTR]
    user.FindWindowExW.restype = w.HWND
    surface = user.FindWindowExW(window, None, 'MyGoSurface', None)
    assert surface, 'Native UI surface not found'
    rect = w.RECT()
    assert user.GetClientRect(w.HWND(surface), c.byref(rect))
    dpi = user.GetDpiForWindow(w.HWND(window))
    x, y = rect.right - round(23 * dpi / 96), round(28 * dpi / 96)
    point = (y << 16) | x
    for message, flags in [(0x200, 0), (0x201, 1), (0x202, 0)]:
        assert user.PostMessageW(w.HWND(surface), message, flags, point), c.WinError(c.get_last_error())
        time.sleep(.06)


def benchmark(runtime, root, mode, seconds, cycles=0, recreate=False, widgets=False, exit_idle=False):
    app = Runtime(runtime, root / uuid.uuid4().hex)
    handle = None
    k32 = c.windll.kernel32
    k32.OpenProcess.restype = w.HANDLE
    try:
        app.data.mkdir(parents=True)
        app.write_settings()
        settings = app.data / 'settings.json'
        state = json.loads(settings.read_text(encoding='utf-8'))
        state.update(StartupSignatureEnabled=False, StartupGreetingEnabled=False, WeatherCity='上海',
                     WeatherAlertsEnabled=False, AllowedSources=['LyricIsland.Verify13'])
        settings.write_text(json.dumps(state), encoding='utf-8')
        version = json.loads((app.runtime / 'Assets/RuntimeFiles.json').read_text(encoding='utf-8'))['version']
        (app.data / 'center-release-notes.json').write_text(json.dumps(dict(version=version)), encoding='utf-8')
        app.launch(env={'MYGO_GPU': '0'} if mode == 'software' else {})
        time.sleep(2)
        assert not app.request('state')['islandVisible']
        before = sample(app.process)
        warm_center = None
        child = lambda: next((p['pid'] for p in children(app) if p['name'] == 'LyricIsland.Center.exe'), None)
        if widgets:
            assert app.request('widgetAdd', {'kind': 'note', 'size': 'medium', 'style': 'original'})['success']
            pid = wait(child, 'warm desktop host')
            wait(lambda: widget_windows(pid), 'warm desktop widget')
            assert not center_window(pid)
            warm = k32.OpenProcess(0x410, False, pid)
            assert warm, c.WinError()
            try:
                time.sleep(4)
                _, committed, working = sample(SimpleNamespace(_handle=warm))
                warm_center = dict(privateBytes=committed, workingSetBytes=working)
            finally:
                k32.CloseHandle(w.HANDLE(warm))
        started = time.monotonic()
        assert app.request('settings', {'page': 'overview'})['success']
        pid = wait(child, 'center child')
        window = wait(lambda: center_window(pid), 'visible center')
        startup_ms = (time.monotonic() - started) * 1000
        handle = k32.OpenProcess(0x410, False, pid)
        assert handle, c.WinError()
        center = SimpleNamespace(_handle=handle)

        def phase(visible=True, gone=False):
            time.sleep(2 if visible else 4)
            if gone:
                wait(lambda: not child(), 'idle center exit', seconds=8)
                code = w.DWORD()
                assert k32.GetExitCodeProcess(w.HANDLE(handle), c.byref(code)) and code.value == 35, code.value

            def samples():
                if gone:
                    assert not child(), 'idle center unexpectedly relaunched'
                    return [(0, 0, 0), sample(app.process)]
                return [sample(center), sample(app.process)]

            begin = samples()
            rows = []
            for _ in range(seconds):
                time.sleep(1)
                assert center_window(pid) == (window if visible else None)
                if widgets:
                    assert len(widget_windows(pid)) == 1
                rows.append(samples())
            return {name: dict(privateBytes=statistics.median(r[n][1] for r in rows),
                               workingSetBytes=statistics.median(r[n][2] for r in rows),
                               cpuMsPerSecond=(rows[-1][n][0] - begin[n][0]) / seconds)
                    for n, name in enumerate(('center', 'island'))}

        result = dict(islandBeforeOpen=before[1:], firstWindowMs=startup_ms, overview=phase())
        if warm_center is not None:
            result['widgetHostBeforeOpen'] = warm_center
        for page in ('plugins', 'island', 'about', 'widgets', 'timers', 'about'):
            assert app.request('settings', {'page': page})['success']
            wait(lambda: not app.request('state')['pendingPage'], f'{page} acknowledged')
        result['afterPageSwitches'] = phase()
        if cycles:
            result['cycles'] = []
            hide_center(window)
            wait(lambda: not center_window(pid), 'quick close', seconds=2)
            assert app.request('settings', {'page': 'overview'})['success']
            wait(lambda: center_window(pid) == window, 'quick reopen reuses window', seconds=2)
            wait(lambda: not app.request('state')['pendingPage'], 'quick reopen acknowledged', seconds=2)
            result['quickReopenReusedWindow'] = True
        for _ in range(cycles):
            hide_center(window)
            wait(lambda: not center_window(pid), 'closed to tray', seconds=8)
            hidden = phase(visible=False, gone=exit_idle)
            assert bool(user.IsWindow(w.HWND(window))) is not (recreate or exit_idle)
            started = time.monotonic()
            page = 'plugins' if len(result['cycles']) % 2 else 'island'
            assert app.request('settings', {'page': page})['success']
            if exit_idle:
                k32.CloseHandle(w.HANDLE(handle))
                handle = None
                old_pid = pid
                pid = wait(child, 'fresh center after idle exit', seconds=8)
                assert pid != old_pid
                handle = k32.OpenProcess(0x410, False, pid)
                assert handle, c.WinError()
                center = SimpleNamespace(_handle=handle)
            reopened = wait(lambda: center_window(pid), 'same resident process reopened', seconds=8)
            reopen_ms = (time.monotonic() - started) * 1000
            assert (reopened != window) == (recreate or exit_idle)
            window = reopened
            wait(lambda: not app.request('state')['pendingPage'], 'reopened page acknowledged', seconds=8)
            result['cycles'].append(dict(hidden=hidden, reopened=phase(), reopenMs=reopen_ms,
                                         recreated=recreate or exit_idle, processExited=exit_idle, requestedPage=page))
        assert app.request('prepareUpdateExit')['success']
        app.process.wait(timeout=20)
        return result
    finally:
        if handle:
            k32.CloseHandle(w.HANDLE(handle))
        if app.process and app.process.poll() is None:
            try:
                app.request('prepareUpdateExit')
                app.process.wait(timeout=10)
            except (OSError, AssertionError, TimeoutError):
                pass
        app.stop()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('runtime', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--mode', nargs='+', default=['default'])
    parser.add_argument('--seconds', type=int, default=6)
    parser.add_argument('--baseline', type=Path)
    parser.add_argument('--runs', type=int, default=1)
    parser.add_argument('--cycles', type=int, default=0)
    parser.add_argument('--recreate', action='store_true', help='expect candidate to release and rebuild its HWND')
    parser.add_argument('--widgets', action='store_true', help='exercise one live desktop widget during all phases')
    parser.add_argument('--exit-idle', action='store_true', help='expect the candidate with no widgets to exit')
    args = parser.parse_args()
    if args.exit_idle and args.widgets:
        parser.error('The center must stay resident while a desktop widget is present')
    args.report.parent.mkdir(parents=True, exist_ok=True)
    results = {}
    variants = [('baseline', args.baseline), ('candidate', args.runtime)] if args.baseline else [('candidate', args.runtime)]
    for trial in range(args.runs):
        for label, runtime in (variants if trial % 2 == 0 else variants[::-1]):
            for mode in args.mode:
                row = benchmark(runtime, args.report.parent / 'center-data', mode, args.seconds, args.cycles,
                                args.recreate and label == 'candidate', args.widgets, args.exit_idle and label == 'candidate')
                results.setdefault(label, {}).setdefault(mode, []).append(row)
                args.report.write_text(json.dumps(results, indent=2), encoding='utf-8')
                print(label, mode, row, flush=True)
