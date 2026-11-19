"""Exercise the packaged native music-card glow without touching user data."""
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
    app.data.mkdir(parents=True)
    app.write_settings()
    config = app.data / 'settings.json'
    settings = json.loads(config.read_text(encoding='utf-8'))
    settings.update(Animation=False, AutoCheckUpdatesEnabled=False, StartupSignatureEnabled=False,
                    StartupGreetingEnabled=False, IslandMaterial='Acrylic', IslandSizeMode='Classic',
                    IslandColorSource='Custom', IslandDynamicSeedColor='#985F53',
                    FontSize=19, LyricColorFromCover=False, LyricProgressColor='#DEE8FF',
                    ProgressBarGlow=True, WeatherAlertsEnabled=False)
    config.write_text(json.dumps(settings), encoding='utf-8')

    def update(key, value):
        reply = app.request('update', {'key': key, 'value': value})
        assert reply['success'], reply

    def preview(draft, name):
        reply = app.request('musicLayoutPreview', draft)
        assert reply['success'], reply
        png = base64.b64decode(reply.pop('png'), validate=True)
        (out / (name + '.png')).write_bytes(png)
        return reply, Image.open(io.BytesIO(png)).convert('RGBA')

    def presented(name):
        time.sleep(.3)
        state = app.request('state')
        assert state['islandVisible'] and not state['lastError'], state['lastError']
        path = (out / (name + '.png')).resolve()
        reply = app.request('debugSnapshot', str(path))
        assert reply['success'], reply
        return Image.open(path).convert('RGBA')

    def visible_delta(a, b):
        return ImageChops.difference(a.convert('RGB'), b.convert('RGB')).convert('L').point(
            lambda v: v if v >= 3 else 0)

    try:
        state = app.launch(args=['--demo'], env={'LYRICISLAND_FONT_PROBE': '1'})
        assert state['settings']['MusicLyricGlow'] is True
        assert app.request('testCompactFrame', {'line': '总好于那日我没有 没有遇过某某',
                                                'translation': '翻译保留原样，不跟着发光'})['success']
        compact_on = presented('compact-on')
        update('MusicLyricGlow', False)
        compact_off = presented('compact-off')
        assert visible_delta(compact_off, compact_on).getbbox() is None
        assert app.request('expand')['success']
        draft = state['settings']['MusicCustomLayout']
        cases = {}
        for layout in ('SideBySide', 'LyricFocus'):
            update('MusicExpandedLayout', layout)
            update('MusicLyricGlow', False)
            geom, off = preview(draft, layout + '-off')
            update('MusicLyricGlow', True)
            after, on = preview(draft, layout + '-on')
            assert after == geom and geom['hasTrack'], geom
            delta = visible_delta(off, on)
            bbox = delta.getbbox()
            assert bbox and delta.getextrema()[1] > 6, (layout, bbox)
            scale = on.width / (geom['width'] + geom['padding'] * 2)
            lyric = next(e for e in geom['elements'] if e['id'] == 3)
            # Only the current line may change in this settled demo fixture.
            y0 = (geom['padding'] + lyric['y'] - 12) * scale
            y1 = (geom['padding'] + lyric['y'] + lyric['h'] + 12) * scale
            assert bbox[1] >= y0 - 1 and bbox[3] <= y1 + 1, (bbox, y0, y1, lyric)
            cases[layout] = {'differenceBoundsPx': bbox, 'peakLumaDelta': delta.getextrema()[1]}
        state = app.request('state')
        assert state['settings']['MusicLyricGlow'] and state['settings']['ProgressBarGlow']
        assert state['expanded']
        presented('expanded-presented')
        return {'success': True, 'compactUnchanged': True, 'independentProgressGlow': True, 'cases': cases}
    finally:
        app.stop()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    report = run(args.runtime.resolve(), args.output.resolve())
    (args.output / 'report.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report))
