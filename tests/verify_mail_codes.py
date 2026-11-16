"""Synthetic QQ mail toast -> read-only Windows-store adapter -> OTP island."""
import argparse
import json
import os
from pathlib import Path
import sqlite3
import time
import traceback
import uuid
from verify_pomodoro import Runtime


def run(args):
    report = dict(success=False, synthetic=True, actualMailRead=False, checks=[])
    root = args.report.parent / ('mail-fixture-' + uuid.uuid4().hex)
    root.mkdir(parents=True)
    database = root / 'notifications.db'
    db = sqlite3.connect(database)
    db.executescript('CREATE TABLE NotificationHandler(RecordId INTEGER,PrimaryId TEXT);'
                     'CREATE TABLE Notification(Id INTEGER,HandlerId INTEGER,ArrivalTime INTEGER,Type TEXT,[Order] INTEGER,Payload BLOB);'
                     "INSERT INTO NotificationHandler VALUES(1,'QQ');")
    db.close()
    os.environ['LYRICISLAND_NOTIFICATION_DB'] = str(database.resolve())
    app = Runtime(args.binary_dir, root / 'data')
    def wait(predicate, timeout=8):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            state = app.request('state')
            if predicate(state):
                return state
            time.sleep(.1)
        raise AssertionError({k: state.get(k) for k in ('notificationState','verificationAlertVisible','verificationCodeLength','lastError')})
    try:
        app.data.mkdir()
        app.write_settings()
        path = app.data / 'settings.json'
        settings = json.loads(path.read_text(encoding='utf-8'))
        settings.update(NotificationsEnabled=True, PauseNotifications=False,
                        StartupSignatureEnabled=False, StartupGreetingEnabled=False,
                        IslandResidentEnabled=True, Animation=False,
                        AllowedSources=['LyricIsland.Verify13'])
        path.write_text(json.dumps(settings), encoding='utf-8')
        app.start(preserve_settings=True)
        wait(lambda s: s['notificationState']['access'] == 'compatible')
        for number, mode in enumerate(('Classic','Mini'), 1):
            assert app.request('update', {'key':'IslandSizeMode','value':mode})['success']
            assert app.request('update', {'key':'PauseNotifications','value':False})['success']
            wait(lambda s: not s['notificationState']['paused'])
            if app.request('state')['islandEnabled']:
                assert app.request('toggleVisibility')['success']
            wait(lambda s: not s['islandEnabled'] and not s['islandVisible'])
            xml = '<toast><visual><binding template="ToastGeneric"><text>QQ邮箱新邮件</text><text>服务登录，您的验证码是 a7B2c9，5 分钟内有效。</text></binding></visual></toast>'
            if mode == 'Mini':
                xml = xml.replace('a7B2c9', 'D4e5F6')
            arrival = time.time_ns() // 100 + 116444736000000000
            db = sqlite3.connect(database)
            db.execute('INSERT INTO Notification VALUES(?,1,?,\'toast\',?,?)', (number,arrival,number,xml.encode()))
            db.commit()
            db.close()
            state = wait(lambda s: s['verificationAlertVisible'])
            assert state['verificationCodeLength'] == 6
            assert state['islandVisible'] and state['backgroundOpacity'] == 1
            assert not state['notificationHistory'], 'OTP must never be persisted as notification history'
            report['checks'].append(mode + ': QQ mail mixed-case OTP reached dedicated visible island; no history persisted')
            assert app.request('update', {'key':'PauseNotifications','value':True})['success']
            wait(lambda s: not s['verificationAlertVisible'] and s['residentIdle'] and not s['islandEnabled'])
        report['success'] = True
    except Exception:
        report['error'] = traceback.format_exc()
        raise
    finally:
        app.stop()
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary-dir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    print(json.dumps(run(parser.parse_args()), ensure_ascii=False))
