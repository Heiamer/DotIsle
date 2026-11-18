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
                        IslandResidentEnabled=True,  # Retired transparent preference; ignored.
                        AllowedSources=['LyricIsland.Verify13'])
        path.write_text(json.dumps(settings), encoding='utf-8')
        state = app.start(preserve_settings=True)
        assert not state['settings']['IslandKeepVisibleEnabled'] and 'IslandResidentEnabled' not in state['settings']
        state_when(lambda s: not s['islandVisible'])
        report['checks'].append('Default off: idle island stays hidden')
        for mode in ('Classic', 'Mini'):
            setting('IslandSizeMode', mode)
            setting('IslandKeepVisibleEnabled', True)
            def idle(s):
                return s['islandVisible'] and s['residentIdle'] and not s['idleClickThrough'] and s['backgroundOpacity'] == 1
            state_when(lambda s: idle(s) and s['residentContent']['page'] == 'time')
            setting('IslandResidentShowTime', False)
            setting('IslandResidentShowSystem', True)
            state_when(lambda s: s.get('residentContent', {}).get('page') == 'system')
            setting('IslandResidentShowTime', True)
            state_when(lambda s: s.get('residentContent', {}).get('page') == 'time')
            state_when(lambda s: s.get('residentContent', {}).get('page') == 'system', timeout=12)
            assert app.request('testPointerClick')['success']
            expanded = state_when(lambda s: s['expanded'] and s['residentContent']['page'] == 'system')
            if not any(d['connected'] for d in expanded['systemMetrics']['wirelessDevices']):
                assert expanded['residentContent']['expandedDeviceRows'] == 0, expanded
            shot = app.data / f'{mode}-dashboard.png'
            assert app.request('debugSnapshot', str(shot))['success'] and shot.stat().st_size > 0
            width = expanded['islandGeometry']['card']['w']
            height = expanded['islandGeometry']['card']['h']
            assert app.request('testPointerClick', {'x': (width - 34) / width, 'y': 28 / height})['success']
            state_when(lambda s: not s['expanded'])
            setting('IslandResidentShowSystem', False)
            report['checks'].append(mode + ': configured rotation, live CPU/memory page, expanded dashboard')
            time.sleep(6.5)
            state = app.request('state')
            assert idle(state), state
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
            state_when(idle)
            setting('PauseNotifications', False)
            assert app.request('notificationPreview')['success']
            state_when(lambda s: s['notificationVisible'] and s['backgroundOpacity'] == 1)
            state_when(idle, timeout=20)
            assert app.request('toggleVisibility')['success']
            state_when(lambda s: not s['islandVisible'] and not s['islandEnabled'])
            assert app.request('toggleVisibility')['success']
            state_when(idle)
            report['checks'].append(mode + ': idle beyond auto-hide deadline; timer and notification hand-offs; manual hide/show')
        assert app.request('prepareUpdateExit')['success']
        app.process.wait(timeout=20)
        app.stop()
        state = app.start(preserve_settings=True)
        assert state['settings']['IslandKeepVisibleEnabled']
        state_when(idle)
        setting('IslandKeepVisibleEnabled', False)
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
