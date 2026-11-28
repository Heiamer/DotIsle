"""Exercise real one-minute reminders in an isolated portable runtime."""
import argparse
import json
import time
import uuid
from pathlib import Path

from ctypes import wintypes as w
from verify_gesture_practice import input_window, user
from verify_pomodoro import Runtime


def verify(runtime, report):
    app = Runtime(runtime.resolve(), report.parent / ('reminders-' + uuid.uuid4().hex))
    checks = []

    def configure(id, **value):
        return app.request('reminderPluginConfigure', dict(id=id, **value))

    def expect(result):
        assert result['success'], result
        return result

    def quit_cleanly():
        expect(app.request('prepareUpdateExit'))
        app.process.wait(timeout=10)
        app.stop()

    def visible(id, timeout=80):
        started = time.monotonic()
        state = {}
        while time.monotonic() - started < timeout:
            state = app.request('state')
            if (state['reminderPluginVisible'] == id and state['islandVisible']
                    and 'testAlertRect' in state and (id != 'hydration' or 'testWaterLayout' in state)):
                return state, round(time.monotonic() - started, 2)
            time.sleep(.10)
        raise AssertionError((id, state.get('screenTime'), state.get('lastError')))

    def click_rect(hwnd, r):
        user.GetDpiForWindow.argtypes = [w.HWND]
        dpi = max(96, user.GetDpiForWindow(hwnd)) / 96
        x = round((r['x']+r['w']/2)*dpi)
        y = round((r['y']+r['h']/2)*dpi)
        position = (x & 0xffff) | ((y & 0xffff) << 16)
        assert user.PostMessageW(hwnd, 0x201, 1, position)
        assert user.PostMessageW(hwnd, 0x202, 0, position)

    def water_geometry(state):
        r = state['testAlertRect']
        g = state['testWaterLayout']
        assert r['h'] == 148 and r['w'] >= 306, state
        track, later, record = (g[k] for k in ('track','later','record'))
        assert track['y'] + track['h'] + 12 <= min(later['y'], record['y']), g
        assert later['x'] + later['w'] + 8 <= record['x'], g
        assert record['y'] + record['h'] + 12 <= r['y'] + r['h'], g
        return g

    try:
        app.data.mkdir(parents=True)
        app.write_settings()
        path = app.data / 'settings.json'
        settings = json.loads(path.read_text(encoding='utf-8'))
        settings.update(StartupSignatureEnabled=False, StartupGreetingEnabled=False,
                        WeatherAlertsEnabled=False, Animation=False,
                        AllowedSources=['LyricIsland.Verify13'])
        path.write_text(json.dumps(settings), encoding='utf-8')
        state = app.start(preserve_settings=True)
        for id in ('sedentary', 'hydration'):
            assert not state['reminderPlugins'][id]['enabled'] and state['reminderPlugins'][id]['minutes'] is None, state
            assert not configure(id, enabled=True)['success']
            for bad in (0, 1441, 1.5, '1'):
                assert not configure(id, minutes=bad)['success']
            assert not expect(configure(id, minutes=1))['reminderPlugins'][id]['enabled']
            expect(configure(id, enabled=True))
        checks.append('Defaults off; explicit numeric value and enable required; range and type validated')
        assert not configure('hydration', goalMl=1600, servingMl=1700)['success']
        expect(configure('hydration', goalMl=1600, servingMl=175))
        assert expect(app.request('hydrationRecord'))['hydrationToday']['drankMl'] == 175
        quit_cleanly()
        state = app.start(preserve_settings=True)
        assert all(p['enabled'] and p['minutes'] == 1 for p in state['reminderPlugins'].values())
        assert state['reminderPlugins']['hydration']['goalMl'] == 1600
        assert state['reminderPlugins']['hydration']['servingMl'] == 175
        assert state['hydrationToday']['drankMl'] == 175
        checks.append('Configured total and serving; explicit logging survived restart')
        checks.append('Both configurations persisted across clean restart')
        stand, stand_seconds = visible('sedentary')
        assert stand_seconds >= 57, stand_seconds
        assert stand['testAlertRect']['h'] == 104
        expect(configure('sedentary', enabled=False))
        water, water_seconds = visible('hydration', 20)
        g = water_geometry(water)
        hwnd = input_window(app.process.pid)
        assert hwnd
        click_rect(hwnd, g['record'])
        time.sleep(.3)
        assert app.request('state')['hydrationToday']['drankMl'] == 350
        click_rect(hwnd, g['record'])
        time.sleep(.3)
        assert app.request('state')['hydrationToday']['drankMl'] == 350
        assert app.request('state')['reminderPlugins']['hydration']['enabled']
        checks.append('Both native cards appeared; target geometry and button gaps verified; click records a serving exactly once')

        later, _ = visible('hydration')
        click_rect(hwnd, water_geometry(later)['later'])
        time.sleep(.3)
        state = app.request('state')
        assert 'testAlertRect' not in state and state['hydrationToday']['drankMl'] == 350
        timeout_card, _ = visible('hydration')
        water_geometry(timeout_card)
        end = time.monotonic() + 18
        while time.monotonic() < end and 'testAlertRect' in app.request('state'):
            time.sleep(.15)
        state = app.request('state')
        assert 'testAlertRect' not in state and state['hydrationToday']['drankMl'] == 350
        expect(configure('hydration', enabled=False))
        checks.append('Later and automatic close do not log a serving')
        seen = {'sedentary': stand_seconds, 'hydrationAfterStand': water_seconds}
        quit_cleanly()
        report.write_text(json.dumps(dict(success=True, checks=checks, visibleAfterSeconds=seen), indent=2), encoding='utf-8')
    finally:
        app.stop()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    options = parser.parse_args()
    verify(options.runtime, options.report.resolve())
