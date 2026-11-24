"""Exercise the packaged window lifecycle without the live user's processes."""
import argparse
import ctypes as c
import json
import time
import uuid
from ctypes import wintypes as w
from pathlib import Path

from benchmark_center import center_window
from verify_pomodoro import Runtime
from verify_unified_center import children, stop, user, wait, widget_windows


def verify(runtime, report):
    app = Runtime(runtime, report.parent / 'center-data' / uuid.uuid4().hex)
    checks = []

    def center_pid():
        matches = [p['pid'] for p in children(app) if p['name'] == 'LyricIsland.Center.exe']
        assert len(matches) <= 1, matches
        return matches[0] if matches else None

    def open_center(**page):
        assert app.request('settings', page)['success']
        pid = wait(center_pid, 'one center child', seconds=8)
        window = wait(lambda: center_window(pid), 'center visible', seconds=8)
        wait(lambda: not app.request('state')['pendingPage'], 'page request acknowledged', seconds=8)
        return pid, window

    def close_center(pid, window):
        # Exercise window lifetime, not a caption hit through the current page/modal.
        assert user.PostMessageW(w.HWND(window), 0x10, 0, 0), c.WinError()
        wait(lambda: not center_window(pid), 'center hidden', seconds=3)
        wait(lambda: not user.IsWindow(w.HWND(window)), 'center window unloaded', seconds=8)

    try:
        app.data.mkdir(parents=True)
        app.write_settings()
        path = app.data / 'settings.json'
        settings = json.loads(path.read_text(encoding='utf-8'))
        settings.update(StartupSignatureEnabled=False, StartupGreetingEnabled=False, WeatherCity='Shanghai',
                        WeatherAlertsEnabled=False, AllowedSources=['LyricIsland.Verify13'])
        path.write_text(json.dumps(settings), encoding='utf-8')
        manifest = app.runtime / 'Assets/RuntimeFiles.json'
        version = json.loads(manifest.read_text(encoding='utf-8'))['version']
        (app.data / 'center-release-notes.json').write_text(json.dumps({'version': version}), encoding='utf-8')
        app.launch()
        pid, window = open_center(action='todoEdit')
        close_center(pid, window)
        assert center_pid() == pid, 'closing a window with a todo draft must not exit the process'
        new_pid, new_window = open_center(page='todos')
        assert new_pid == pid and new_window != window
        checks.append('Open todo draft keeps the process alive after window release')

        # Cancel the draft with the native Escape key. The reopened app then has
        # nothing to retain except a running timer owned by the island process.
        surface = user.FindWindowExW(new_window, None, 'MyGoSurface', None)
        assert surface
        for message in (0x100, 0x101):
            assert user.PostMessageW(w.HWND(surface), message, 0x1B, 0), c.WinError()
        time.sleep(.3)
        timer = app.create({'mode': 'countdown', 'duration': 60, 'title': 'Tray test'})
        close_center(new_pid, new_window)
        wait(lambda: not center_pid(), 'draft dismissed and no widget host required', seconds=8)
        assert app.timer(timer)['running'] is True
        checks.append('Clean idle exit leaves the island countdown running')

        widget = app.request('widgetAdd', {'kind': 'note', 'size': 'medium', 'style': 'original'})
        assert widget['success'], widget
        pid = wait(center_pid, 'warm widget host', seconds=10)
        wait(lambda: widget_windows(pid), 'real widget window', seconds=10)
        assert not center_window(pid)
        new_pid, window = open_center(page='overview')
        assert new_pid == pid
        close_center(pid, window)
        assert center_pid() == pid and len(widget_windows(pid)) == 1
        assert app.request('widgetRemove', {'id': widget['id']})['success']
        wait(lambda: not center_pid(), 'removing last widget releases unused background host', seconds=10)
        assert open_center(page='plugins')[0] != pid
        checks.append('Widget stays visible through window release; removing the last widget releases the host')

        fresh_pid = center_pid()
        close_center(fresh_pid, center_window(fresh_pid))
        wait(lambda: not center_pid(), 'idle center exit before warm-host case', seconds=10)
        widget = app.request('widgetAdd', {'kind': 'note', 'size': 'medium', 'style': 'original'})
        assert widget['success'], widget
        pid = wait(center_pid, 'widget-only host', seconds=10)
        wait(lambda: widget_windows(pid), 'widget-only window', seconds=10)
        assert not center_window(pid)
        assert app.request('widgetRemove', {'id': widget['id']})['success']
        wait(lambda: not center_pid(), 'widget-only host exits without a center visit', seconds=10)
        assert open_center(page='overview')[0] != pid
        checks.append('A widget-only launch also exits after removing the last widget')
        return dict(success=True, checks=checks)
    finally:
        stop(app)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    args.report.parent.mkdir(parents=True, exist_ok=True)
    result = verify(args.runtime, args.report)
    args.report.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(result), flush=True)
