"""Synthetic QQ toast -> DLL -> native banner, without private messages."""
import argparse
from html import escape
import json
from pathlib import Path
import sqlite3
import time
import uuid

from PIL import Image

from verify_pomodoro import Runtime
from verify_unified_center import stop, wait


def run(runtime, output):
    output = output.resolve()
    root = output / ('fixture-' + uuid.uuid4().hex)
    root.mkdir(parents=True)
    database = root / 'notifications.db'
    with sqlite3.connect(database) as db:
        db.executescript('CREATE TABLE NotificationHandler(RecordId INTEGER,PrimaryId TEXT);'
                         'CREATE TABLE Notification(Id INTEGER,HandlerId INTEGER,ArrivalTime INTEGER,Type TEXT,[Order] INTEGER,Payload BLOB);'
                         "INSERT INTO NotificationHandler VALUES(1,'Tencent.QQ.NT');")
    local = root / 'Local'
    app = Runtime(runtime, root / 'data')
    serial = 0

    def emit(handler, src, name):
        nonlocal serial
        serial += 1
        xml = ('<toast><visual><binding template="ToastGeneric"><text>Fixture sender</text>'
               f'<text>Test message {serial}</text><image placement="appLogoOverride" src="{escape(src)}"/>'
               '</binding></visual></toast>')
        with sqlite3.connect(database) as db:
            db.execute("INSERT INTO Notification VALUES(?,?,?,'toast',?,?)",
                       (serial, handler, time.time_ns() // 100 + 116444736000000000, serial, xml.encode()))
        deadline = time.monotonic() + 10
        path = output / (name + '.png')
        while time.monotonic() < deadline:
            state = app.request('state')
            if state['notificationVisible'] and state['islandVisible']:
                for _ in range(4):
                    time.sleep(.075)
                    if app.request('debugSnapshot', str(path))['success']:
                        with Image.open(path) as image:
                            rgb = image.convert('RGBA')
                            # Match saturated test colors, ignoring card/background pixels.
                            return [sum(1 for p in rgb.getdata() if p[3] > 220 and p[i] > 200 and
                                        all(p[j] < 60 for j in range(3) if j != i)) for i in range(3)]
            time.sleep(.075)
        raise AssertionError('Native notification did not present: ' + name)

    try:
        app.data.mkdir()
        app.write_settings()
        path = app.data / 'settings.json'
        settings = json.loads(path.read_text(encoding='utf-8'))
        settings.update(NotificationsEnabled=True, VerificationCodeEnabled=False,
                        NotificationWhitelistEnabled=True, NotificationAllowedApps=['QQNT'],
                        StartupSignatureEnabled=False, StartupGreetingEnabled=False, Animation=False)
        path.write_text(json.dumps(settings), encoding='utf-8')
        app.launch(env={'LYRICISLAND_NOTIFICATION_DB': str(database), 'LOCALAPPDATA': str(local)})
        wait(lambda: app.request('state')['notificationState']['access'] == 'compatible', 'fixture reader')
        captures = {}
        for color, (handler, app_name, name, extension) in enumerate([
            (1, 'QQNT', 'qq-first', 'png'), (1, 'QQNT', 'qq-second', 'jpg')
        ]):
            file = local / 'Tencent' / app_name / (name + '.' + extension)
            file.parent.mkdir(parents=True, exist_ok=True)
            Image.new('RGB', (64, 64), tuple(240 if ch == color else 16 for ch in range(3))).save(file)
            counts = emit(handler, file.as_uri(), name)
            assert counts[color] > 180 and all(v < 80 for i, v in enumerate(counts) if i != color), (name, counts)
            captures[name] = counts
            wait(lambda: not app.request('state')['notificationVisible'], name + ' dismissed')
        captures['fallback'] = emit(1, 'https://invalid.example/unused.png', 'fallback')
        assert all(v < 80 for v in captures['fallback']), captures['fallback']
        return dict(success=True, synthetic=True, colorPixels=captures)
    finally:
        stop(app)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    report = run(args.runtime, args.output)
    (args.output / 'report.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report))
