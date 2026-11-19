"""Compare the editor's native preview against the real expanded island."""
import argparse
import base64
import io
import json
import time
import uuid
from pathlib import Path

from PIL import Image, ImageChops, ImageGrab, ImageStat

from benchmark_center import center_window
from verify_pomodoro import Runtime
from verify_unified_center import children, stop, wait


def run(runtime, output):
    app = Runtime(runtime, output / ('data-' + uuid.uuid4().hex))
    output.mkdir(parents=True, exist_ok=True)

    def update(key, value):
        reply = app.request('update', dict(key=key, value=value))
        assert reply['success'], reply

    def capture(draft, name):
        reply = app.request('musicLayoutPreview', draft)
        assert reply['success'], reply
        png = base64.b64decode(reply.pop('png'), validate=True)
        (output / f'{name}.png').write_bytes(png)
        image = Image.open(io.BytesIO(png)).convert('RGBA')
        assert image.getchannel('A').getbbox(), 'empty preview'
        return reply, image

    def card_pixels(image, x, y, width, height, scale):
        return image.crop(tuple(round(v * scale) for v in (x, y, x + width, y + height)))

    def present(name, geometry):
        state = app.request('state')
        assert state['expanded'] and state['islandVisible'], state['islandGeometry']
        assert not state['lastError'], state['lastError']
        card = state['islandGeometry']['card']
        assert abs(card['h'] - geometry['height']) < .01, (card, geometry)
        assert abs(card['w'] - geometry['width']) < .01, (card, geometry)
        file = output / f'{name}-host.png'
        assert app.request('debugSnapshot', str(file.resolve()))['success']
        actual = Image.open(file).convert('RGBA')
        dpi = round(actual.height / max(332, card['h'] + 72) * 4) / 4
        return card_pixels(actual, card['x'], card['y'], card['w'], card['h'], dpi)

    env = {'LYRICISLAND_FONT_PROBE': '1'}
    try:
        app.data.mkdir(parents=True)
        app.write_settings()
        config = app.data / 'settings.json'
        settings = json.loads(config.read_text(encoding='utf-8'))
        settings.update(StartupSignatureEnabled=False, StartupGreetingEnabled=False,
                        IslandMaterial='Acrylic', IslandSizeMode='Classic',
                        CompactWidth=352, AutoCompactMaxWidth=420, FontSize=19,
                        Animation=False, WeatherAlertsEnabled=False)
        config.write_text(json.dumps(settings), encoding='utf-8')
        version = json.loads((app.runtime / 'Assets/RuntimeFiles.json').read_text(encoding='utf-8'))['version']
        (app.data / 'center-release-notes.json').write_text(json.dumps(dict(version=version)), encoding='utf-8')
        state = app.launch(args=['--demo'], env=env)
        assert state['settings']['ProgressBarStyle'] == 'Normal', state['settings']['ProgressBarStyle']
        assert app.request('expand')['success']
        time.sleep(1)
        draft = state['settings']['MusicCustomLayout']
        assert draft['barStyle'] == 'Normal' and not draft['enabled']
        builtin, _ = capture(draft, 'built-in-preview')
        assert builtin['width'] != 400 and builtin['hasTrack'] and not builtin['glass']
        assert app.request('settings', {'page': 'layout'})['success']
        pid = wait(lambda: next((p['pid'] for p in children(app) if p['name'] == 'LyricIsland.Center.exe'), None), 'center child')
        hwnd = wait(lambda: center_window(pid), 'layout editor window')
        wait(lambda: not app.request('state')['pendingPage'], 'layout editor route acknowledgment')
        time.sleep(2)
        editor_capture = False
        try:
            ImageGrab.grab(window=hwnd).save(output / 'layout-editor.png')
            editor_capture = True
        except OSError as error:
            print(f'Control center screen capture unavailable: {error}')

        # Move title and progress, including a previously out-of-bounds placement.
        # Applying must keep the preset frame and still affect the actual
        # HWND's presentation, not just save a JSON field or resize the preview.
        draft.update(enabled=True, gap=12, elements=[dict(element=1, x=100, y=128, width=180),
                                                     dict(element=6, x=24, y=288, width=340)])
        draft_geo, offscreen = capture(draft, 'draft-preview')
        assert (draft_geo['width'], draft_geo['height']) == (builtin['width'], builtin['height']), (builtin, draft_geo)
        seek = next(e for e in draft_geo['elements'] if e['id'] == 6)
        assert seek['y'] < 288 and seek['y'] + seek['h'] < draft_geo['height'], seek
        title = next(e for e in draft_geo['elements'] if e['id'] == 1)
        assert abs(title['x'] - 100 * draft_geo['width'] / 400) < .01 and title['y'] == 128, title
        assert app.request('state')['settings']['MusicCustomLayout']['enabled'] is False
        update('MusicCustomLayout', draft)
        time.sleep(1)
        actual = present('applied', draft_geo)
        actual.save(output / 'applied-island.png')
        scale = offscreen.width / (draft_geo['width'] + 2 * draft_geo['padding'])
        expected = card_pixels(offscreen, draft_geo['padding'], draft_geo['padding'],
                               draft_geo['width'], draft_geo['height'], scale)
        expected = expected.resize(actual.size)
        expected.save(output / 'preview-card.png')
        # Time/progress may advance between captures. Compare the relocated title
        # where the preview and presented island should share stable pixels.
        title_mask = tuple(round(v * actual.width / draft_geo['width']) for v in
                           (title['x'], title['y'], title['x'] + title['w'], title['y'] + title['h']))
        diff = ImageChops.difference(expected.crop(title_mask), actual.crop(title_mask))
        rgb_delta = ImageStat.Stat(diff).mean[:3]
        assert max(rgb_delta) < 12, rgb_delta
        update('MusicCustomLayout', dict(draft, enabled=False))
        reverted, _ = capture(dict(draft, enabled=False), 'disabled-preview')
        assert reverted['elements'] == builtin['elements'], 'disabled preview did not revert to built-in geometry'
        stop(app)
        # Demo mode intentionally does not write settings. Test persistence in the
        # normal executable with an empty, isolated player source list.
        settings['AllowedSources'] = ['LyricIsland.Verify13']
        config.write_text(json.dumps(settings), encoding='utf-8')
        app.launch()
        update('MusicCustomLayout', draft)
        stop(app)
        restarted = app.launch()['settings']
        assert restarted['MusicCustomLayout'] == draft
        assert restarted['ProgressBarStyle'] == 'Normal'
        for key in ('ProgressBarGlow', 'MusicLyricGlow'):
            assert restarted[key] is True
            update(key, False)
        assert not any(app.request('state')['settings'][key] for key in ('ProgressBarGlow', 'MusicLyricGlow'))
        return dict(success=True, defaultBar='Normal', width=draft_geo['width'], height=draft_geo['height'],
                    appliedTitle=title, relocatedTitleMeanRgbDelta=rgb_delta, persistedAfterRestart=True,
                    editorWindowCapture=editor_capture)
    finally:
        stop(app)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    report = run(args.runtime, args.output)
    (args.output / 'report.json').write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding='utf-8')
    print(json.dumps(report))
