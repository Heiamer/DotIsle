"""Read real Windows data through the shipped C/JSON module ABI in isolation.

No player controls, notification content, device pairing or installed settings are
changed. Network weather reads are opt-in; normal regression tests remain offline.
"""
import argparse
import ctypes
import json
import os
from pathlib import Path
import time
import traceback
import shutil
from verify_pomodoro import Runtime


class Buffer(ctypes.Structure):
    _fields_ = [('ptr', ctypes.c_void_p), ('length', ctypes.c_size_t)]


class Module:
    def __init__(self, directory, name):
        self.library = ctypes.CDLL(str(directory / f'dotisle_{name}.dll'))
        self.library.dotisle_service_call.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
        self.library.dotisle_service_call.restype = Buffer
        self.library.dotisle_service_free.argtypes = [Buffer]
        self.library.dotisle_service_free.restype = None

    def call(self, method, **args):
        request = json.dumps(dict(method=method, args=args)).encode('utf-8')
        raw = self.library.dotisle_service_call(request, len(request))
        try:
            assert raw.ptr and raw.length <= 4 * 1024 * 1024
            reply = json.loads(ctypes.string_at(raw.ptr, raw.length))
        finally:
            self.library.dotisle_service_free(raw)
        assert reply['ok'], reply
        return reply['value']


def run(args):
    directory = args.binary_dir.resolve()
    os.environ['DOTISLE_MODULE_DIR'] = str(directory)
    combase = ctypes.WinDLL('combase')
    combase.RoInitialize.argtypes = [ctypes.c_uint]
    combase.RoInitialize.restype = ctypes.c_long
    initialized = combase.RoInitialize(1) >= 0
    report = dict(success=False, checks={}, limitations=[])
    try:
        names = ('core', 'desktop', 'focus', 'todos', 'music', 'lyrics',
                 'notifications', 'history', 'automation', 'devices', 'osd',
                 'sound', 'widgets', 'themes', 'glow')
        modules = {name: Module(directory, name) for name in names}
        for name, module in modules.items():
            metadata = module.call('describe')
            assert metadata['id'] == name and metadata['abi'] == 1
            health = module.call('health')
            assert health['success'], health
            report['checks'][f'module:{name}'] = health

        widgets = modules['widgets']
        handle = widgets.call('metrics.new')['handle']
        value = {}
        deadline = time.monotonic() + 25
        while time.monotonic() < deadline:
            value = widgets.call('metrics.poll', handle=handle)
            if value.get('cpuPercent') is not None and value.get('wirelessUpdatedAt', 0):
                break
            time.sleep(.1)
        assert 0 <= value['cpuPercent'] <= 100, value
        assert 0 <= value['memoryPercent'] <= 100, value
        assert 0 < value['memoryUsedGib'] <= value['memoryTotalGib'], value
        if value.get('batteryPercent') is not None:
            assert 0 <= value['batteryPercent'] <= 100, value
        else:
            report['limitations'].append('System battery is absent or not reported; no percentage fabricated.')
        assert value['wirelessStatus'] == 'ready', value
        assert value['wirelessUpdatedAt'] > 0, value
        for device in value['wirelessDevices']:
            assert device['connected'] and device['id']
            if device.get('batteryPercent') is not None:
                assert 0 <= device['batteryPercent'] <= 100
        report['checks']['systemMetrics'] = value
        widgets.call('metrics.drop', handle=handle)

        placements = [dict(id='disk', kind='storage', config={'drive': 'C:'}, x=0, y=0),
                      dict(id='clock', kind='worldClock', config={'city': '纽约'}, x=0, y=0)]
        if args.network:
            placements += [dict(id=f'weather-{i}', kind='weather', config={'city': city}, x=0, y=0)
                           for i, city in enumerate(('上海', '北京', 'Beijing'))]
        handle = widgets.call('data.new', path='')['handle']
        value = widgets.call('data.poll', handle=handle, placements=placements)['value']
        assert value['disk']['total'] > value['disk']['free'] >= 0, value
        assert len(value['clock']['clock']) == 5, value
        report['checks']['diskAndWorldClock'] = {key: value[key] for key in ('disk', 'clock', 'focusStats')}
        if args.network:
            deadline = time.monotonic() + 40
            while time.monotonic() < deadline:
                value = widgets.call('data.poll', handle=handle, placements=placements)['value']
                if all(value[p['id']].get('temperature') is not None or value[p['id']].get('error')
                       for p in placements if p['kind'] == 'weather'):
                    break
                time.sleep(.1)
            weather = {p['config']['city']: value[p['id']] for p in placements if p['kind'] == 'weather'}
            report['checks']['weather'] = weather
            for city, data in weather.items():
                assert isinstance(data.get('temperature'), (int, float)), (city, data)
                assert data['source'] in ('QQ 天气', '今日头条天气', 'Open-Meteo', 'MET Norway',
                                          'DWD / Bright Sky', 'wttr.in', '7Timer'), (city, data)
                assert isinstance(data['time'], int)
                assert not data['stale'] and isinstance(data['precipitation'], list)
                assert data['precipitationForecastAvailable'] is True, (city, data)
                assert data['forecastThrough'] >= data['time'] + 3 * 3600, (city, data)
                assert data['forecastResolutionMinutes'] in (60, 180), (city, data)
        widgets.call('data.drop', handle=handle)

        osd = modules['osd']
        handle = osd.call('hub.new')['handle']
        osd.call('hub.resync', handle=handle)
        osd.call('hub.poll', handle=handle, mic=True, touch=True)
        status = osd.call('hub.status', handle=handle)
        assert isinstance(status['caps'], bool) and isinstance(status['num'], bool), status
        report['checks']['systemOsd'] = status
        osd.call('hub.drop', handle=handle)

        report['checks']['mediaSessions'] = modules['music'].call('diagnostics')

        # The notification SQLite dependency resolves beside the host EXE. Run
        # this check in the real native host, not beside the Python interpreter.
        app = Runtime(directory, args.report.parent / 'native-data')
        try:
            app.data.mkdir(parents=True, exist_ok=True)
            app.write_settings()
            settings_path = app.data / 'settings.json'
            settings = json.loads(settings_path.read_text(encoding='utf-8'))
            settings.update(PauseNotifications=True, NotificationsEnabled=True)
            settings_path.write_text(json.dumps(settings), encoding='utf-8')
            state = app.start(preserve_settings=True)
            assert state['moduleReady'], state['moduleErrors']
            deadline = time.monotonic() + 8
            while time.monotonic() < deadline:
                state = app.request('state')
                if state['notificationState'].get('access') == 'compatible' and state['notificationState'].get('paused'):
                    break
                time.sleep(.1)
            notification = state['notificationState']
            assert notification['access'] == 'compatible' and notification['paused'], notification
            report['checks']['notificationDatabase'] = {
                'access': notification['access'], 'source': notification['source'], 'paused': notification['paused']}
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                state = app.request('state')
                if state['screenTime']['todayObservedMs'] > 0:
                    break
                time.sleep(.1)
            assert state['screenTime']['todayObservedMs'] > 0, state['screenTime']
            assert 0 <= state['screenTime']['todayMs'] <= state['screenTime']['todayObservedMs'], state['screenTime']
            observed = state['screenTime']['todayObservedMs']
            report['checks']['nativeState'] = {
                'moduleReady': state['moduleReady'], 'foregroundReadable': not state['foreground']['unreadable'],
                'screenTime': state['screenTime'], 'lockWatch': state['lockWatch']}
            todo = app.request('todoSave', {'title': '数据回归待办', 'content': '重启保留',
                                           'date': '2030-01-01', 'time': '09:00', 'daily': False})
            assert todo['success'], todo
            todo_id = todo['id']
            assert app.request('todoComplete', {'id': todo_id})['success']
            quick = app.request('quickReminderPreview', {'text': '20分钟后喝水'})
            assert quick['success'] and quick['previewToken'], quick
            commit = {'previewToken': quick['previewToken'], 'clientRequestId': 'data-' + quick['previewToken']}
            assert app.request('quickReminderCommit', commit)['success']
            before_count = len(app.request('state')['todos'])
            assert app.request('quickReminderCommit', commit)['success']
            assert len(app.request('state')['todos']) == before_count
            rule = app.request('appRuleSave', {'name': '数据回归规则', 'executablePath': r'C:\Windows\System32\notepad.exe',
                                             'message': '休息一下', 'cooldownMinutes': 30, 'thresholdMinutes': 60,
                                             'trigger': 'foregroundDaily', 'enabled': False})
            assert rule['success'], rule
            rule_id = rule['id']
            template = app.request('workflowTemplateSave', {'name': '数据回归流程',
                                  'stages': [{'name': '学习', 'seconds': 60}, {'name': '休息', 'seconds': 60}]})
            assert template['success'], template
            template_id = template['id']
            assert app.request('overviewWidgetSet', {'kind': 'note', 'config': {'text': '便签数据回归'}})['success']
            if args.network or args.updates:
                channels = {}
                for channel in ('stable', 'beta'):
                    assert app.request('updateChannel', channel)['success']
                    assert app.request('checkUpdate')['success']
                    deadline = time.monotonic() + 55
                    while time.monotonic() < deadline:
                        state = app.request('state')
                        if state['softwareUpdate']['phase'] != 'checking':
                            break
                        time.sleep(.1)
                    update = state['softwareUpdate']
                    assert state['updateChannel'] == channel, state['updateChannel']
                    assert update['phase'] in ('idle', 'available'), update
                    assert update['version'] and update['source'] == 'GitHub', update
                    channels[channel] = update
                report['checks']['nativeUpdateChannels'] = channels
            plugin = args.report.parent / 'isolated-plugin'
            plugin.mkdir(exist_ok=True)
            source = Path(__file__).resolve().parents[1] / 'examples/plugins/hello/manifest.json'
            shutil.copy2(source, plugin / 'manifest.json')
            (plugin / 'state.json').write_text('{"counter":17,"progress":0.5}', encoding='utf-8')
            assert app.request('pluginInstall', {'path': str(plugin.resolve())})['success']
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                state = app.request('state')
                text = json.dumps(state['plugins'], ensure_ascii=False)
                if '17' in text:
                    break
                time.sleep(.1)
            assert '17' in text, state['plugins']
            (plugin / 'state.json').write_text('{"counter":23,"progress":0.75}', encoding='utf-8')
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                state = app.request('state')
                text = json.dumps(state['plugins'], ensure_ascii=False)
                if '23' in text:
                    break
                time.sleep(.1)
            assert '23' in text, state['plugins']
            report['checks']['pluginFileData'] = {'initial': 17, 'updated': 23, 'liveRefresh': True}
            assert app.request('pluginRemove', {'id': 'example.hello'})['success']
            assert app.request('prepareUpdateExit')['success']
            app.process.wait(timeout=20)
            state = app.start(preserve_settings=True)
            assert state['settings']['PauseNotifications']
            if args.network or args.updates:
                assert state['updateChannel'] == 'beta'
            assert state['screenTime']['todayObservedMs'] >= observed, state['screenTime']
            report['checks']['screenTimePersistence'] = {'beforeMs': observed, 'restartedMs': state['screenTime']['todayObservedMs']}
            assert next(t for t in state['todos'] if t['id'] == todo_id)['completed']
            assert next(r for r in state['appRules'] if r['id'] == rule_id)['enabled'] is False
            assert len(next(t for t in state['templates'] if t['id'] == template_id)['stages']) == 2
            assert next(w for w in state['overviewWidgets'] if w['kind'] == 'note')['config']['text'] == '便签数据回归'
            assert app.request('todoDelete', {'id': todo_id})['success']
            rule = next(r for r in state['appRules'] if r['id'] == rule_id)
            assert app.request('appRuleAction', {'id': rule_id, 'action': 'delete', 'revision': rule['revision']})['success']
            assert app.request('workflowTemplateDelete', template_id)['success']
            assert app.request('overviewWidgetRemove', {'kind': 'note'})['success']
            report['checks']['localFeatureData'] = {'todoCompletedAndPersisted': True, 'quickReminderIdempotent': True,
                'appRulePersisted': True, 'workflowTemplatePersisted': True, 'notePersisted': True, 'deletions': True}
            assert app.request('prepareUpdateExit')['success']
            app.process.wait(timeout=20)
        finally:
            app.stop()
        report['success'] = True
    except Exception:
        report['error'] = traceback.format_exc()
        raise
    finally:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
        if initialized:
            combase.RoUninitialize()
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary-dir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--network', action='store_true')
    parser.add_argument('--updates', action='store_true', help='Check native update channels independently of weather')
    result = run(parser.parse_args())
    print(json.dumps({'success': result['success'], 'checks': len(result['checks'])}))
