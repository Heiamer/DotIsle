"""Exercise the playing desktop widget and expanded island through Windows SMTC."""
import argparse
import json
import statistics
import subprocess
import time
import uuid
from pathlib import Path

from PIL import Image, ImageChops

from benchmark_idle_reminders import sample
from verify_pomodoro import Runtime
from verify_unified_center import host, stop, wait, widget_windows


def verify(runtime, fixture, output):
    app = Runtime(runtime, output / uuid.uuid4().hex)
    publisher = None
    try:
        app.data.mkdir(parents=True)
        app.write_settings()
        path = app.data / 'settings.json'
        settings = json.loads(path.read_text(encoding='utf-8'))
        settings.update(AllowedSources=['verify_smtc'], StartupSignatureEnabled=False,
                        StartupGreetingEnabled=False, IslandMaterial='LightField', IslandSizeMode='Classic')
        path.write_text(json.dumps(settings), encoding='utf-8')
        cover_a, cover_b = (app.data / f'cover-{i}.png' for i in range(2))
        Image.new('RGB', (240, 240), (237, 48, 73)).save(cover_a)
        Image.new('RGB', (240, 240), (46, 120, 234)).save(cover_b)
        app.launch()
        with (app.data / 'smtc.log').open('wb') as log:
            publisher = subprocess.Popen([str(fixture.resolve()), str(app.data), str(cover_a), str(cover_b), '32'],
                                         stdout=log, stderr=log, creationflags=subprocess.CREATE_NO_WINDOW)

        def track(title):
            state = app.request('state')
            return state if state['isPlaying'] and state['title'] == title else None

        wait(lambda: track('First track'), 'first SMTC track', seconds=10)
        widget = app.request('widgetAdd', {'kind': 'music', 'size': 'medium', 'style': 'original'})
        assert widget['success'], widget
        pid = wait(lambda: host(app), 'music widget host', seconds=10)
        wait(lambda: len(widget_windows(pid)) == 1, 'music widget visible', seconds=10)
        assert app.request('expand')['success']
        time.sleep(2)
        first = track('First track')
        assert first and first['islandVisible'] and first['expanded'] and not first['lastError']
        a, b = (app.data / f'island-{i}.png' for i in range(2))
        assert app.request('debugSnapshot', str(a))['success']
        started = time.monotonic()
        cpu = sample(app.process)[0]
        rows = []
        for _ in range(8):
            time.sleep(1)
            rows.append(sample(app.process))
        elapsed = time.monotonic() - started
        last = app.request('state')
        assert last['isPlaying'] and last['position'] > first['position']
        assert last['rendererGeneration'] == first['rendererGeneration']
        assert last['renderFrames'] > first['renderFrames']
        wait(lambda: track('Second track'), 'updated SMTC title', seconds=15)
        time.sleep(2)
        assert app.request('debugSnapshot', str(b))['success']
        with Image.open(a) as before, Image.open(b) as after:
            changed = ImageChops.difference(before.convert('RGB'), after.convert('RGB')).getbbox()
            assert changed is not None, 'expanded island did not reflect the updated track'
        assert len(widget_windows(pid)) == 1 and not app.request('state')['desktopWidgetError']
        assert app.request('widgetRemove', {'id': widget['id']})['success']
        wait(lambda: not widget_windows(pid), 'music widget removed', seconds=10)
        return dict(success=True, source='isolated Windows SMTC', islandPrivateBytes=statistics.median(r[1] for r in rows),
                    islandMaxSamplePrivateBytes=max(r[1] for r in rows), islandCpuMsPerSecond=(rows[-1][0]-cpu)/elapsed,
                    islandFramesPerSecond=(last['renderFrames']-first['renderFrames'])/elapsed,
                    progressSeconds=last['position']-first['position'], screenshots=[str(a),str(b)])
    finally:
        if publisher and publisher.poll() is None:
            publisher.terminate()
            publisher.wait(timeout=10)
        try:
            stop(app)
        finally:
            app.stop()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    result = verify(args.runtime, args.fixture, args.report.parent / 'widget-smtc-data')
    args.report.write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(result))
