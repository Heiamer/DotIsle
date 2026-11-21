"""Synthetic Feishu/DingTalk toast -> production reader -> island; WeChat stays on its local database."""
import argparse, json, os, sqlite3, time, uuid
from pathlib import Path
from verify_pomodoro import Runtime


def run(a):
    a.report.parent.mkdir(parents=True, exist_ok=True)
    root = a.report.parent / ('message-fixture-' + uuid.uuid4().hex)
    root.mkdir()
    database = root / 'notifications.db'
    with sqlite3.connect(database) as db:
        db.executescript('CREATE TABLE NotificationHandler(RecordId INTEGER,PrimaryId TEXT);'
                        'CREATE TABLE Notification(Id INTEGER,HandlerId INTEGER,ArrivalTime INTEGER,Type TEXT,[Order] INTEGER,Payload BLOB);'
                        "INSERT INTO NotificationHandler VALUES(1,'com.bytedance.Feishu'),(2,'DingTalk'),(3,'Tencent.WeChat');")
    os.environ['LYRICISLAND_NOTIFICATION_DB'] = str(database.resolve())
    app = Runtime(a.binary_dir, root / 'data')
    report = dict(success=False, synthetic=True, actualChatRead=False, checks=[])
    serial = 0

    def wait(predicate, timeout=8):
        until = time.monotonic() + timeout
        while time.monotonic() < until:
            state = app.request('state')
            if predicate(state):
                return state
            time.sleep(.1)
        raise AssertionError(('Synthetic message assertion timed out', state.get('notificationState'), state.get('moduleErrors')))

    def insert(handler, body, title='合成发件人'):
        nonlocal serial
        serial += 1
        xml = f'<toast><visual><binding template="ToastGeneric"><text>{title}</text><text>{body}</text></binding></visual></toast>'
        arrival = time.time_ns() // 100 + 116444736000000000
        with sqlite3.connect(database) as db:
            db.execute("INSERT INTO Notification VALUES(?,?,?,'toast',?,?)", (serial,handler,arrival,serial,xml.encode()))

    def history():
        return app.request('state')['notificationHistory']

    try:
        app.data.mkdir()
        app.write_settings()
        path = app.data / 'settings.json'
        settings = json.loads(path.read_text(encoding='utf-8'))
        settings.update(NotificationsEnabled=True, PauseNotifications=False,
                        NotificationWhitelistEnabled=True, NotificationAllowedApps=['飞书','钉钉','微信'],
                        WeChatMessageHintsEnabled=True, WeComMessageHintsEnabled=False,
                        StartupSignatureEnabled=False, StartupGreetingEnabled=False,
                        VerificationCodeEnabled=False, Animation=False)
        path.write_text(json.dumps(settings),encoding='utf-8')
        app.start(preserve_settings=True)
        wait(lambda s:s['notificationState']['access']=='compatible')
        before = len(history())
        wait(lambda s: ((s.get('notificationState',{}).get('messageHints') or {}).get('wechat') or {}).get('source') == 'local-wcdb')
        insert(3, '有新消息')
        time.sleep(.8)
        assert app.request('state')['notificationContent'] is None, 'WeChat placeholder produced a banner'
        insert(3, '微信系统通知验证')
        time.sleep(.8)
        assert app.request('state')['notificationContent'] is None, 'WeChat Windows toast bypassed the local conversation database'
        assert len(history()) == before, 'WeChat body was stored in history'
        report['checks'].append('WeChat: content comes only from the local conversation database; Windows toasts are ignored')
        for handler, app_id in ((1,'Feishu'),(2,'DingTalk')):
            before = len(history())
            for body in ('合成通知正文甲','另一条合成通知正文乙'):
                insert(handler, body)
                expected = before + 1
                state = wait(lambda s:len(s['notificationHistory'])==expected)
                assert any(item.get('appId')==app_id or item.get('app_id')==app_id for item in state['notificationHistory']), 'Canonical app identity missing'
                assert any(item.get('title')=='合成发件人' and item.get('body')==body for item in state['notificationHistory']), 'Sender or body was replaced by a placeholder'
                before += 1
            insert(handler, '')
            time.sleep(2.3)
            assert len(history()) == before, 'Empty placeholder was shown'
            report['checks'].append(app_id+': sender and body preserved, two messages within cooldown retained, empty notice discarded')
        assert app.request('update',{'key':'PauseNotifications','value':True})['success']
        wait(lambda s:s['notificationState']['paused'])
        before = len(history())
        insert(1,'暂停期间的合成通知')
        time.sleep(.8)
        assert len(history()) == before
        assert app.request('update',{'key':'PauseNotifications','value':False})['success']
        time.sleep(1)
        assert len(history()) == before, 'Paused content replayed'
        report['checks'].append('Pause suppresses content without replay')
        report['success'] = True
        return report
    finally:
        app.stop()
        a.report.write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')


if __name__ == '__main__':
    p=argparse.ArgumentParser()
    p.add_argument('--binary-dir',type=Path,required=True)
    p.add_argument('--report',type=Path,required=True)
    print(json.dumps(run(p.parse_args()),ensure_ascii=False))
