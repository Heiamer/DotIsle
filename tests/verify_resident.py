"""Exercise real idle/task residency in an isolated native instance."""
import argparse
import json
from pathlib import Path
import time
import traceback
import uuid
from verify_pomodoro import Runtime


def run(args):
    report = {'success': False, 'checks': [], 'snapshots': {}}
    app = Runtime(args.binary_dir, args.report.parent / ('resident-data-' + uuid.uuid4().hex))
    def state_when(predicate, timeout=8):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            state = app.request('state')
            if predicate(state):
                return state
            time.sleep(.1)
        raise AssertionError(state)
    def setting(key, value):
        reply = app.request('update', {'key': key, 'value': value})
        assert reply['success'], reply
    try:
        app.data.mkdir(parents=True, exist_ok=True)
        app.write_settings()
        path = app.data / 'settings.json'
        settings = json.loads(path.read_text(encoding='utf-8'))
        settings.update(StartupSignatureEnabled=False, StartupGreetingEnabled=False,
                        WeatherAlertsEnabled=False, Animation=False,
                        AllowedSources=['LyricIsland.Verify13'])
        path.write_text(json.dumps(settings), encoding='utf-8')
        state = app.start(preserve_settings=True)
        assert not state['settings']['IslandResidentEnabled']
        state_when(lambda s: not s['islandVisible'])
        report['checks'].append('Default off: idle island stays hidden')
        for mode in ('Classic', 'Mini'):
            setting('IslandSizeMode', mode)
            setting('IslandResidentEnabled', True)
            state = state_when(lambda s: s['islandWindowPresent'] and s['residentIdle'] and not s['islandVisible'] and s['idleClickThrough'])
            time.sleep(6.5)
            state = app.request('state')
            assert state['islandWindowPresent'] and not state['islandVisible'] and state['residentIdle'] and state['backgroundOpacity'] == 0 and state['idleClickThrough'], state
            report['snapshots'][mode + ':idle'] = {k: state[k] for k in ('islandVisible', 'islandWindowPresent', 'residentIdle', 'backgroundOpacity', 'idleClickThrough')}
            timer = app.create({'mode': 'pomodoro', 'focus': 60, 'rest': 60, 'rounds': 1})
            state = state_when(lambda s: not s['residentIdle'] and s['backgroundOpacity'] == 1)
            assert state['islandVisible']
            # Restart clears the manually held editor while retaining the live timer.
            assert app.request('prepareUpdateExit')['success']
            app.process.wait(timeout=20)
            app.stop()
            app.start(preserve_settings=True)
            assert app.act(timer, 'resume')['success']
            state_when(lambda s: s['islandVisible'] and not s['residentIdle'] and s['backgroundOpacity'] == 1)
            assert app.act(timer, 'delete')['success']
            state_when(lambda s: s['residentIdle'] and not s['islandVisible'] and s['backgroundOpacity'] == 0 and s['idleClickThrough'])
            setting('PauseNotifications', False)
            assert app.request('notificationPreview')['success']
            state_when(lambda s: s['notificationVisible'] and s['backgroundOpacity'] == 1)
            setting('PauseNotifications', True)
            state_when(lambda s: s['residentIdle'] and s['islandWindowPresent'] and not s['islandVisible'] and s['idleClickThrough'])
            assert app.request('toggleVisibility')['success']
            state_when(lambda s: not s['islandVisible'] and not s['islandEnabled'])
            assert app.request('toggleVisibility')['success']
            state_when(lambda s: s['islandWindowPresent'] and s['residentIdle'] and not s['islandVisible'] and s['idleClickThrough'])
            report['checks'].append(mode + ': idle beyond auto-hide deadline; timer and notification hand-offs; manual hide/show')
        assert app.request('prepareUpdateExit')['success']
        app.process.wait(timeout=20)
        app.stop()
        state = app.start(preserve_settings=True)
        assert state['settings']['IslandResidentEnabled']
        state_when(lambda s: s['islandWindowPresent'] and s['residentIdle'] and not s['islandVisible'] and s['idleClickThrough'])
        setting('IslandResidentEnabled', False)
        state_when(lambda s: not s['islandVisible'])
        report['checks'].append('Resident preference survives restart; disabling restores auto-hide')
        assert app.request('prepareUpdateExit')['success']
        app.process.wait(timeout=20)
        report['success'] = True
    except Exception:
        report['error'] = traceback.format_exc()
        raise
    finally:
        app.stop()
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary-dir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    print(json.dumps(run(parser.parse_args()), ensure_ascii=False))
