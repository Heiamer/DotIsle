"""Drive the shipped island's window procedures on a private pipe and data directory."""
import argparse
import ctypes
from ctypes import wintypes as w
import json
from pathlib import Path
import uuid
import time

from verify_pomodoro import Runtime
from verify_unified_center import WindowCallback, stop, user, wait

user.GetClassNameW.argtypes = [w.HWND, w.LPWSTR, ctypes.c_int]
user.GetDpiForWindow.argtypes = [w.HWND]
user.GetDpiForWindow.restype = w.UINT
user.PostMessageW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
user.PostMessageW.restype = w.BOOL


def input_window(pid):
    handles = []

    @WindowCallback
    def visit(hwnd, _):
        owner = w.DWORD()
        user.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid:
            name = ctypes.create_unicode_buffer(128)
            user.GetClassNameW(hwnd, name, len(name))
            if name.value == 'LyricIsland.Native.DirectComposition.v1':
                handles.append(hwnd)
        return True

    user.EnumWindows(visit, 0)
    return handles[0] if len(handles) == 1 else None


def run(runtime, output):
    app = Runtime(runtime, output / ('data-' + uuid.uuid4().hex))
    app.data.mkdir(parents=True)
    app.write_settings()  # Simulate an existing user with the old tutorial marked seen.
    settings = app.data / 'settings.json'
    settings.write_text(json.dumps(dict(json.loads(settings.read_text(encoding='utf-8')),
                                       Animation=False, IslandSizeMode='Classic',
                                       ClickAnywhereExpand=True, HoverExpand=False,
                                       SwipeActivities=True, SwipeUpHide=True,
                                       AllowedSources=['DotIsle.VerifyPractice'])), encoding='utf-8')
    owner = uuid.uuid4().hex

    def lesson(action, id=owner):
        return app.request('gesturePractice', {'action': action, 'id': id})

    def step(target):
        wait(lambda: app.request('state').get('gesturePractice', {}).get('step') == target, target, 8)

    def drag(dx, dy):
        time.sleep(.35)  # Let the native shape settle after the preceding click.
        r = app.request('state')['islandGeometry']['capsule']
        x, y = r['x'] + r['w'] / 2, r['y'] + r['h'] / 2
        scale = max(96, user.GetDpiForWindow(hwnd)) / 96
        for msg, buttons, tx, ty in [(0x201, 1, x, y), (0x200, 1, x+dx, y+dy), (0x202, 0, x+dx, y+dy)]:
            pos = (round(tx*scale) & 0xffff) | ((round(ty*scale) & 0xffff) << 16)
            assert user.PostMessageW(hwnd, msg, buttons, pos)
            time.sleep(.03)

    try:
        initial = app.launch()
        assert initial['settings']['GestureTutorialSeen'] and not initial['settings']['GesturePracticeSeen']
        hwnd = wait(lambda: input_window(app.process.pid), 'isolated island input window')
        assert lesson('start')['success']
        step('clickExpand')
        wait(lambda: app.request('state')['islandVisible'], 'practice island visible', 8)
        assert not lesson('finish')['success']
        assert not lesson('skip', 'unrelated-session')['success']
        assert not app.request('expand')['success']
        assert app.request('testPointerClick', {'x': .5, 'y': .52})['success']
        step('clickCollapse')
        assert app.request('testPointerClick', {'x': .5, 'y': .38})['success']
        step('swipe')
        drag(-130, 0)
        try:
            step('hide')
        except AssertionError as err:
            debug = app.request('state')
            raise AssertionError({k: debug.get(k) for k in ('gesturePractice', 'activeComponent', 'components', 'expanded', 'islandGeometry', 'islandVisible', 'lastError')}) from err
        assert app.request('state')['activeComponent'] == 'practice:second'
        drag(0, -102)
        step('recover')
        app.request('toggleVisibility')
        step('recover')  # Non-tray visibility commands must not mark recovery done.
        assert lesson('cancel')['success']
        restored = app.request('state')
        assert 'gesturePractice' not in restored and restored['activeComponent'] == initial['activeComponent']
        assert restored['islandEnabled'] == initial['islandEnabled']
        assert not restored['settings']['GesturePracticeSeen']
        assert lesson('skip')['success']
        stop(app)
        assert app.launch()['settings']['GesturePracticeSeen']
        assert lesson('start')['success'], 'manual replay must remain available'
        assert lesson('cancel')['success']
        return {'success': True, 'checks': ['old tutorial flag ignored', 'click expand and collapse',
                 'drag between cards', 'swipe up', 'tray-only recovery gate', 'cancel restores',
                 'skip survives restart', 'manual replay']}
    finally:
        stop(app)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = run(args.runtime.resolve(), args.output.resolve())
    (args.output / 'report.json').write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(result, ensure_ascii=False))
