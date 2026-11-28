"""Verify fullscreen auto-hide against a real, temporary Win32 window.

Run in an interactive desktop session: the fixture briefly takes foreground focus.
Never moves or constrains the mouse and never closes another application's window.
"""
import argparse
import ctypes as c
from ctypes import wintypes as w
import json
from pathlib import Path
import time
import uuid

from verify_pomodoro import Runtime
from verify_unified_center import WindowCallback, user


class MonitorInfo(c.Structure):
    _fields_ = [('size', w.DWORD), ('monitor', w.RECT), ('work', w.RECT), ('flags', w.DWORD)]


user.MonitorFromPoint.argtypes = [w.POINT, w.DWORD]
user.MonitorFromPoint.restype = w.HANDLE
user.GetMonitorInfoW.argtypes = [w.HANDLE, c.POINTER(MonitorInfo)]
user.GetMonitorInfoW.restype = w.BOOL
user.CreateWindowExW.argtypes = [w.DWORD, w.LPCWSTR, w.LPCWSTR, w.DWORD, c.c_int, c.c_int,
                                 c.c_int, c.c_int, w.HWND, w.HMENU, w.HINSTANCE, c.c_void_p]
user.CreateWindowExW.restype = w.HWND
user.GetForegroundWindow.restype = w.HWND
user.SetForegroundWindow.argtypes = [w.HWND]
user.DestroyWindow.argtypes = [w.HWND]
user.SetWindowPos.argtypes = [w.HWND, w.HWND, c.c_int, c.c_int, c.c_int, c.c_int, w.UINT]
user.PeekMessageW.argtypes = [c.POINTER(w.MSG), w.HWND, w.UINT, w.UINT, w.UINT]
user.DispatchMessageW.argtypes = [c.POINTER(w.MSG)]

KEY = 'HideIslandWhenFullscreen'
ISLAND_WINDOWS = {'LyricIsland.Native.DirectComposition.v1', 'LyricIsland.Native.Visual.v1'}


def island_windows(pid):
    handles = {}

    @WindowCallback
    def visit(hwnd, _):
        process = w.DWORD()
        user.GetWindowThreadProcessId(hwnd, c.byref(process))
        if process.value == pid:
            name = c.create_unicode_buffer(128)
            user.GetClassNameW(hwnd, name, len(name))
            if name.value in ISLAND_WINDOWS:
                handles[name.value] = hwnd
        return True

    user.EnumWindows(visit, 0)
    return handles


def pump():
    message = w.MSG()
    while user.PeekMessageW(c.byref(message), None, 0, 0, 1):
        user.DispatchMessageW(c.byref(message))


def verify(runtime, report):
    app = Runtime(runtime, report.parent / ('fullscreen-' + uuid.uuid4().hex))
    info = MonitorInfo(size=c.sizeof(MonitorInfo))
    assert user.GetMonitorInfoW(user.MonitorFromPoint(w.POINT(0, 0), 1), c.byref(info))
    rect = info.monitor
    fixture = user.CreateWindowExW(0, 'STATIC', 'DotIsle fullscreen verification', 0x80000000,
                                   rect.left, rect.top, 520, 360, None, None, None, None)
    if not fixture:
        raise c.WinError(c.get_last_error())
    checks = []

    def place(full):
        width, height = (rect.right - rect.left, rect.bottom - rect.top) if full else (520, 360)
        assert user.SetWindowPos(fixture, None, rect.left, rect.top, width, height, 0x0040 | 0x0004)
        user.SetForegroundWindow(fixture)
        pump()
        assert user.GetForegroundWindow() == fixture, 'The temporary test window could not get foreground focus'

    def expect(command, value=None):
        reply = app.request(command, value)
        assert reply['success'], reply
        return reply

    def update(key, value):
        expect('update', {'key': key, 'value': value})

    def wait(label, visible, suppressed, enabled, timeout=15):
        last = None
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            pump()
            state = app.request('state')
            handles = island_windows(state['processId'])
            flags = {key: bool(user.IsWindowVisible(hwnd)) for key, hwnd in handles.items()}
            last = (state['islandVisible'], state['fullscreenSuppressed'], state['islandEnabled'], flags)
            assert user.GetForegroundWindow() == fixture, f'{label}: DotIsle must not activate itself'
            if (last[:3] == (visible, suppressed, enabled) and flags.keys() == ISLAND_WINDOWS
                    and (all(flags.values()) if visible else not flags['LyricIsland.Native.Visual.v1'])
                    and (not suppressed or not any(flags.values()))):
                checks.append(label)
                return
            time.sleep(.04)
        raise AssertionError(f'{label}: {last}')

    try:
        app.data.mkdir(parents=True)
        app.write_settings()
        path = app.data / 'settings.json'
        settings = json.loads(path.read_text(encoding='utf-8'))
        settings.update(IslandKeepVisibleEnabled=True, HiddenDisplay='None', Animation=False,
                        StartupSignatureEnabled=False, StartupGreetingEnabled=False,
                        WeatherAlertsEnabled=False, GestureTutorialSeen=True, GesturePracticeSeen=True)
        path.write_text(json.dumps(settings), encoding='utf-8')
        prior = user.GetForegroundWindow()
        place(True)
        state = app.start(preserve_settings=True)
        assert state['settings'][KEY] is False
        wait('Default off: a real fullscreen foreground window does not hide the island', True, False, True)
        update(KEY, True)
        wait('Turning on the setting hides both native windows without changing focus', False, True, True)
        place(False)
        wait('Exiting fullscreen restores both windows without changing focus', True, False, True)
        update('IslandSizeMode', 'Mini')
        place(True)
        wait('Mini is hidden too', False, True, True)
        update(KEY, False)
        wait('Turning off the setting restores the island while still fullscreen', True, False, True)
        place(False)
        update(KEY, True)
        expect('toggleVisibility')
        wait('Manual hide before fullscreen', False, False, False)
        place(True)
        wait('Fullscreen does not change the manual hide flag', False, True, False)
        place(False)
        wait('Exiting fullscreen does not reveal a manually hidden island', False, False, False)
        expect('prepareUpdateExit')
        app.process.wait(timeout=10)
        app.stop()
        place(True)
        state = app.start(preserve_settings=True)
        assert state['settings'][KEY] is True
        wait('Restart into fullscreen respects the saved setting', False, True, True)
        report.write_text(json.dumps({'success': True, 'checks': checks}, indent=2), encoding='utf-8')
    finally:
        app.stop()
        if user.GetForegroundWindow() == fixture and locals().get('prior'):
            user.SetForegroundWindow(prior)
        user.DestroyWindow(fixture)


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--runtime', type=Path, required=True)
    p.add_argument('--report', type=Path, required=True)
    args = p.parse_args()
    args.report.parent.mkdir(parents=True, exist_ok=True)
    verify(args.runtime, args.report)
