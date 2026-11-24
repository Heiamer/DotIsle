"""Synthetic Feishu/DingTalk toast -> production reader -> island; WeChat Windows toasts are ignored."""
import argparse, json, os, sqlite3, subprocess, time, uuid
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
        assert app.request('state')['settings']['WeChatMessageHintsEnabled'] is False, 'Old opt-in carried over'
        assert app.request('update', {'key':'WeChatMessageHintsEnabled', 'value':True})['success'] is False
        assert app.request('wechatWarningOpen', 'canceled-fixture')['success']
        assert app.request('wechatWarningConfirm', 'canceled-fixture')['success'] is False
        assert app.request('wechatWarningCancel', 'canceled-fixture')['success']
        assert app.request('wechatWarningOpen', 'active-fixture')['success']
        start = time.monotonic()
        time.sleep(9.5)
        if time.monotonic() - start < 9.75:
            assert app.request('wechatWarningConfirm', 'active-fixture')['success'] is False
        time.sleep(max(0, 10.15 - (time.monotonic() - start)))
        assert app.request('state')['settings']['WeChatMessageHintsEnabled'] is False, 'Timeout enabled the reminder without a click'
        assert app.request('wechatWarningConfirm', 'canceled-fixture')['success'] is False
        assert app.request('wechatWarningConfirm', 'active-fixture')['success']
        assert app.request('state')['settings']['WeChatMessageHintsEnabled'] is True
        assert app.request('wechatWarningConfirm', 'active-fixture')['success'] is False, 'Consent request could be reused'
        report['checks'].append('WeChat: migrated off; direct toggle, early and canceled confirmation rejected; ten seconds plus explicit confirmation required')
        before = len(history())
        insert(3, '有新消息')
        time.sleep(.8)
        assert app.request('state')['notificationContent'] is None, 'WeChat placeholder produced a banner'
        insert(3, '微信系统通知验证')
        time.sleep(.8)
        assert app.request('state')['notificationContent'] is None, 'WeChat Windows toast produced a banner'
        assert app.request('state')['testMessageAlert'] is None, 'WeChat Windows toast produced a taskbar-style hint'
        assert len(history()) == before, 'WeChat body was stored in history'
        report['checks'].append('WeChat: Windows toasts do not produce banners or history entries')
        if a.flash_fixture:
            wait(lambda s: s['notificationState']['messageHints']['taskbar'])
            # The unsigned fixture identifies as Weixin.exe. The real WeChat client is never touched.
            proc = subprocess.Popen([str(a.flash_fixture.resolve())])
            try:
                state = wait(lambda s: (s['testMessageAlert'] or {}).get('title') == '微信有新消息')
                assert state['testMessageAlert'] == {'title':'微信有新消息','detail':'','trailing':''}
                assert state['notificationContent'] is None and len(history()) == before
                report['checks'].append('WeChat: real Windows shell flash produced a body-free capsule without history')
            finally:
                if proc.poll() is None:
                    proc.terminate()
                    proc.wait(timeout=5)
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
    p.add_argument('--flash-fixture',type=Path,help='Build fixtures/taskbar_flash.cs as Weixin.exe using csc')
    print(json.dumps(run(p.parse_args()),ensure_ascii=False))
