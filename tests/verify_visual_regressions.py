"""Regressions for the 5.0.8 follow-up: no-jump custom layout, capsule width, thumb keys."""
import argparse
import base64
import io
import json
import time
import uuid
from pathlib import Path

from PIL import Image, ImageChops

from verify_pomodoro import Runtime


def run(runtime, out):
    app = Runtime(runtime, out / ('data-' + uuid.uuid4().hex))
    out.mkdir(parents=True, exist_ok=True)
    app.data.mkdir(parents=True)
    app.write_settings()
    config = app.data / 'settings.json'
    settings = json.loads(config.read_text(encoding='utf-8'))
    settings.update(Animation=False, AutoCheckUpdatesEnabled=False, StartupSignatureEnabled=False,
                    StartupGreetingEnabled=False, IslandSizeMode='Classic', FontSize=14,
                    AutoCompactWidth=True, AutoCompactMaxWidth=420, CompactWidth=260,
                    ProgressBarGlow=True, MusicLyricGlow=True, WeatherAlertsEnabled=False)
    config.write_text(json.dumps(settings), encoding='utf-8')

    def preview(value):
        reply = app.request('musicLayoutPreview', value)
        assert reply['success'], reply
        png = base64.b64decode(reply.pop('png'), validate=True)
        return reply, png

    def image(png):
        return Image.open(io.BytesIO(png)).convert('RGBA')

    try:
        state = app.launch(args=['--demo'], env={'LYRICISLAND_FONT_PROBE': '1'})
        assert app.request('expand')['success']
        time.sleep(1)

        # 1. Enabling the untouched custom layout must keep the current card exactly.
        draft = state['settings']['MusicCustomLayout']
        off, off_png = preview(dict(draft, enabled=False))
        on, on_png = preview(dict(draft, enabled=True))
        assert on['elements'] == off['elements'], 'custom layout reflowed its elements'
        assert (on['width'], on['height']) == (off['width'], off['height'])
        width = image(off_png).width
        top = int(image(off_png).height * .6)
        changed = ImageChops.difference(
            image(off_png).crop((0, 0, width, top)), image(on_png).crop((0, 0, width, top))
        ).getbbox()
        assert changed is None, f'custom layout changed the preview at {changed}'
        (out / 'layout-off.png').write_bytes(off_png)
        (out / 'layout-on.png').write_bytes(on_png)

        # 2. A long lyric must widen the capsule instead of staying at CompactWidth.
        line = 'to know other side of the story before dawn'
        assert app.request('testCompactFrame', {
            'line': line, 'translation': '', 'index': 1, 'position': 60,
        })['success']
        time.sleep(2)
        live = app.request('state')
        auto = live['settings']['AutoCompactWidth']
        limit = live['settings']['AutoCompactMaxWidth']
        assert auto, 'auto width must stay enabled in this fixture'
        assert live['compactTargetWidth'] > 300, live['compactTargetWidth']
        assert live['compactTargetWidth'] <= limit + 0.01, (live['compactTargetWidth'], limit)
        assert abs(live['compactWidth'] - live['compactTargetWidth']) < 2, live['compactWidth']
        shot = out / 'long-line.png'
        assert app.request('debugSnapshot', str(shot.resolve()))['success'] and shot.stat().st_size > 0

        # 3. Every thumb chip key must be accepted by the native setting.
        assert app.request('update', {'key': 'ProgressBarThumbEnabled', 'value': True})['success']
        for key in ('Pill', 'Circle', 'Square', 'Diamond', 'Heart', 'Flower', 'Clam', 'Puffy'):
            reply = app.request('update', {'key': 'ProgressBarThumb', 'value': key})
            assert reply['success'], (key, reply)
        legacy = app.request('update', {'key': 'ProgressBarThumb', 'value': 'Dot'})
        assert legacy['success'] is False, 'the retired Dot key must be rejected'
        assert app.request('update', {'key': 'ProgressBarThumb', 'value': 'Pill'})['success']

        return {'success': True, 'layoutNoJump': True, 'layoutWidth': off['width'],
                'layoutHeight': off['height'], 'compactWidth': live['compactWidth'],
                'compactLimit': limit, 'lyric': live['title'], 'thumbKeys': 8,
                'legacyThumbRejected': True}
    finally:
        app.stop()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    report = run(args.runtime.resolve(), args.output.resolve())
    (args.output / 'report.json').write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding='utf-8')
    print(json.dumps(report, ensure_ascii=True))
