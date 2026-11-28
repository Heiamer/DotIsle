"""Synthetic DesktopToast snapshots and mail bridge checks against an isolated packaged island."""
import argparse
import json
from pathlib import Path
import time
import uuid

from verify_pomodoro import Runtime
from verify_unified_center import stop, wait


def run(runtime, output):
    fixture = output.resolve() / ('fixture-' + uuid.uuid4().hex)
    fixture.mkdir(parents=True)
    snapshots = fixture / 'toasts.json'
    snapshots.write_text('[]', encoding='utf-8')
    toasts = []

    def emit(seq, app, title, body):
        toasts.append(dict(id=seq, time=seq, app=app, aumid=app, texts=[title, body]))
        pending = snapshots.with_suffix('.tmp')
        pending.write_text(json.dumps(toasts, ensure_ascii=False), encoding='utf-8')
        pending.replace(snapshots)

    app = Runtime(runtime, fixture / 'data')
    report = {'success': False, 'synthetic': True, 'liveClientVerified': False, 'checks': []}

    try:
        app.data.mkdir()
        app.write_settings()
        path = app.data / 'settings.json'
        settings = json.loads(path.read_text(encoding='utf-8'))
        settings.update(NotificationsEnabled=True, NotificationWhitelistEnabled=False,
                        NotificationBlacklistEnabled=False, DingTalkMessageHintsEnabled=True,
                        FeishuMessageHintsEnabled=True, VerificationCodeEnabled=True,
                        StartupSignatureEnabled=False, StartupGreetingEnabled=False, Animation=False)
        path.write_text(json.dumps(settings), encoding='utf-8')
        emit(0, 'Feishu', 'Startup sender', 'Must not replay')
        app.launch(env={'LYRICISLAND_DESKTOP_TOAST_FIXTURE': str(snapshots), 'LOCALAPPDATA': str(fixture / 'Local')})
        state = lambda: app.request('state')
        wait(lambda: state()['notificationState'].get('desktopToast', {}).get('access') == 'listening', 'isolated notification reader')
        assert not state()['notificationHistory'], 'Startup toast replayed'
        for seq, provider in enumerate(('DingTalk', 'Feishu'), 1):
            emit(seq, provider, 'Fixture sender', 'Actual fixture message')
            def content_visible():
                seen = state()
                c = seen['notificationContent'] or {}
                return seen['notificationVisible'] and c.get('appId') == provider and c.get('bodyCharacters') == 22
            wait(content_visible, provider + ' content')
            wait(lambda: not state()['notificationVisible'], provider + ' content dismissed')
            report['checks'].append(provider + ' toast retains its actual message body')
        for seq, provider, title in [(3, 'DingTalk', '钉钉'), (4, 'Feishu', '飞书')]:
            emit(seq, provider, title, '您收到了一条新消息。')
            expected = title + '有新消息'
            wait(lambda: (state().get('testMessageAlert') or {}).get('title') == expected,
                 provider + ' metadata-only alert')
            assert not state()['notificationVisible']
            wait(lambda: not state().get('testMessageAlert'), provider + ' metadata alert dismissed')
            report['checks'].append(provider + ' generic toast does not fabricate message content')
        assert app.request('update', {'key': 'PauseNotifications', 'value': True})['success']
        wait(lambda: state()['notificationState']['paused'], 'notification pause')
        emit(5, 'Feishu', 'Paused sender', 'Must not replay')
        time.sleep(1.3)
        assert app.request('update', {'key': 'PauseNotifications', 'value': False})['success']
        wait(lambda: state()['notificationState']['desktopToast']['access'] == 'listening', 'notification resume')
        time.sleep(1.3)
        assert not any(item.get('title') == 'Paused sender' for item in state()['notificationHistory'])
        report['checks'].append('WinRT startup and pause baselines do not replay retained notifications')
        ignored = app.request('mailOtp', {'subject': 'News', 'body': 'Issue 452316 is available'})
        assert ignored == {'success': True, 'shown': False}, ignored
        mail = {'subject': '邮箱登录安全验证', 'body': '您的登录验证码为 452316，请勿转发。'}
        reply = app.request('mailOtp', mail)
        assert reply == {'success': True, 'shown': True}, reply
        wait(lambda: state().get('verificationAlertVisible'), 'mail OTP has a live island card')
        assert state()['verificationCodeLength'] == 6
        report['checks'].append('synthetic new mail invokes the native verification card without touching clipboard')
        report['success'] = True
        return report
    except Exception as exc:
        report['error'] = str(exc)
        raise
    finally:
        try:
            stop(app)
        finally:
            (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(run(args.runtime, args.output), ensure_ascii=False))
