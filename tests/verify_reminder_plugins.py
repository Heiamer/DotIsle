"""Exercise real one-minute reminders in an isolated portable runtime."""
import argparse
import json
import time
import uuid
from pathlib import Path

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
            assert state['reminderPlugins'][id] == {'enabled': False, 'minutes': None}, state
            assert not configure(id, enabled=True)['success']
            for bad in (0, 1441, 1.5, '1'):
                assert not configure(id, minutes=bad)['success']
            assert not expect(configure(id, minutes=1))['reminderPlugins'][id]['enabled']
            expect(configure(id, enabled=True))
        checks.append('Defaults off; explicit numeric value and enable required; range and type validated')
        quit_cleanly()
        state = app.start(preserve_settings=True)
        assert all(p == {'enabled': True, 'minutes': 1} for p in state['reminderPlugins'].values())
        checks.append('Both configurations persisted across clean restart')
        started = time.monotonic()
        seen = {}
        while time.monotonic() - started < 80 and len(seen) < 2:
            state = app.request('state')
            alert = state['reminderPluginVisible']
            # The alert is armed one tick before the native window fades in.
            if alert and state['islandVisible'] and alert not in seen:
                seen[alert] = round(time.monotonic() - started, 2)
                assert seen[alert] >= 57, seen
                expect(configure(alert, enabled=False))
            time.sleep(.20)
        assert set(seen) == {'sedentary', 'hydration'}, (seen, state['screenTime'], state['lastError'])
        assert not any(p['enabled'] for p in app.request('state')['reminderPlugins'].values())
        checks.append('Both reminders reached the visible island; disabling one left the other enabled')
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
