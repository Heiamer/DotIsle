"""Real settings pipe, Windows media session, and detached Control Center launch."""
import argparse
from ctypes import wintypes as w
import json
from pathlib import Path
import shutil
import subprocess
import sys
import time
import uuid

from PIL import Image
from benchmark_center import center_window
from verify_pomodoro import Runtime
from verify_unified_center import Process, children, host, snapshot, stop, user, wait, widget_windows


def assert_detached(pid):
    source = '''import ctypes as c, sys
k=c.WinDLL('kernel32', use_last_error=True)
k.FreeConsole()
if k.AttachConsole(int(sys.argv[1])):
    k.FreeConsole()
    sys.exit('Target has a console')
if c.get_last_error() != 6:
    sys.exit(f'Unexpected AttachConsole error: {c.get_last_error()}')'''
    subprocess.run([sys.executable, '-c', source, str(pid)], check=True,
                   creationflags=subprocess.CREATE_NO_WINDOW, capture_output=True)


def verify(runtime, fixture, output):
    app = Runtime(runtime, output / uuid.uuid4().hex)
    app.data.mkdir(parents=True)
    app.write_settings()
    path = app.data / 'settings.json'
    config = json.loads(path.read_text(encoding='utf-8'))
    config.update(BrowserMediaEnabled=False, IgnoredSources=['Browser'],
                  StartupSignatureEnabled=False, StartupGreetingEnabled=False,
                  WeatherCity='上海', WeatherAlertsEnabled=False)
    path.write_text(json.dumps(config), encoding='utf-8')
    (app.data / 'center-release-notes.json').write_text('{"version":"5.1.1"}', encoding='utf-8')
    pub = None
    try:
        state = app.launch(creationflags=0)
        assert state['moduleReady'], state['moduleErrors']
        assert_detached(app.process.pid)
        # Persist a placement, then exercise the login branch before asking for settings.
        placement = app.request('widgetAdd', {'kind': 'note', 'size': 'medium', 'style': 'original'})
        assert placement['success'], placement
        warm_pid = wait(lambda: host(app), 'initial desktop host')
        wait(lambda: widget_windows(warm_pid), 'initial desktop widget')
        stop(app)
        state = app.launch(creationflags=0)
        assert_detached(app.process.pid)
        warm_pid = wait(lambda: host(app), 'desktop host after login launch')
        assert_detached(warm_pid)
        wait(lambda: widget_windows(warm_pid), 'restored desktop widget')
        assert not center_window(warm_pid), 'login startup must not open Control Center'
        assert app.request('settings', {'page': 'overview'})['success']
        pid = wait(lambda: next((p['pid'] for p in children(app)
                                 if p['name'] == 'LyricIsland.Center.exe'), None), 'center child')
        hwnd = wait(lambda: center_window(pid), 'visible center window', seconds=15)
        assert not user.IsIconic(w.HWND(hwnd)), 'GUI must not start minimized'
        assert_detached(pid)
        wait(lambda: not app.request('state')['pendingPage'], 'center acknowledged page', seconds=15)
        for p in snapshot(2, 0, Process, 'Process32'):
            if p['parent'] in [app.process.pid, pid]:
                assert p['name'].lower() not in ('cmd.exe', 'conhost.exe', 'openconsole.exe', 'powershell.exe', 'pwsh.exe'), p
        result = {'loginMainDetached': True, 'loginWidgetHostDetached': True,
                  'centerVisibleOnRequest': True, 'centerDetached': True}
        if fixture:
            sample = app.data / 'DotIsle-browser-fixture.exe'
            shutil.copy2(fixture, sample)
            covers = [app.data / f'cover-{i}.png' for i in range(2)]
            for i, path in enumerate(covers):
                Image.new('RGB', (16, 16), (64 * i, 110, 200)).save(path)
            with (app.data / 'smtc.log').open('wb') as log:
                pub = subprocess.Popen([str(sample), str(app.data), *map(str,covers), '60'],
                                       stdout=log, stderr=log, creationflags=subprocess.CREATE_NO_WINDOW)
            state = lambda: app.request('state')
            is_fixture = lambda s: s['player'] == 'Browser' and s['title'] in ('First track', 'Second track') and s['isPlaying']
            time.sleep(1)
            assert pub.poll() is None and not is_fixture(state())
            for enabled in [True, False, True]:
                reply = app.request('update', {'key': 'BrowserMediaEnabled', 'value': enabled})
                assert reply['success'], reply
                wait(lambda: is_fixture(state()) == enabled, f'browser media set to {enabled}', seconds=12)
            result['browserMediaLiveToggle'] = True
        try:
            reading = wait(lambda: (weather if not weather.get('loading', False) else None)
                           if (weather := app.request('state')['weather']) else None, 'weather provider', seconds=45)
            result['weather'] = {k: reading[k] for k in ('city','temperature','source','stale','error') if k in reading}
        except AssertionError as exc:
            result['weather'] = {'error': str(exc)}
        return result
    finally:
        if pub is not None and pub.poll() is None:
            pub.terminate()
            pub.wait(timeout=10)
        if app.process and app.process.poll() is None:
            try:
                stop(app)
            finally:
                app.stop()


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('--runtime', type=Path, required=True)
    p.add_argument('--fixture', type=Path)
    p.add_argument('--report', type=Path, required=True)
    a = p.parse_args()
    a.report.parent.mkdir(parents=True, exist_ok=True)
    results = verify(a.runtime, a.fixture, a.report.parent)
    a.report.write_text(json.dumps(results, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    print(json.dumps(results, ensure_ascii=True))
