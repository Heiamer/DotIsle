"""A/B production presentation under isolated, reproducible active workloads."""
import argparse
import json
import statistics
import time
import uuid
from pathlib import Path

from benchmark_idle_reminders import sample
from verify_pomodoro import Runtime


def run(runtime, root, scenario, material, seconds, *, lyric_glow=None):
    app = Runtime(runtime, root / f'{scenario}-{material}-{uuid.uuid4().hex}')
    is_music = scenario == 'music'
    try:
        app.data.mkdir(parents=True)
        app.write_settings()
        path = app.data / 'settings.json'
        settings = json.loads(path.read_text(encoding='utf-8'))
        settings.update(StartupSignatureEnabled=False, StartupGreetingEnabled=False,
                        WeatherAlertsEnabled=False, IslandMaterial=material, IslandSizeMode='Classic',
                        AllowedSources=['LyricIsland.Verify13'])
        if is_music and lyric_glow is not None:
            settings['MusicLyricGlow'] = lyric_glow
        path.write_text(json.dumps(settings), encoding='utf-8')
        env = {'DOTISLE_TRACE_GPU': '1'}
        if is_music:
            env['LYRICISLAND_FONT_PROBE'] = '1'  # Preserve isolated settings in demo mode.
        app.launch(args=['--demo'] if is_music else [], env=env)
        identifier = None if is_music else app.create({'mode': 'pomodoro', 'focus': 240, 'rest': 240, 'rounds': 2})
        assert app.request('expand')['success']
        time.sleep(4)

        def validate(state):
            assert state['expanded'] and state['islandVisible'], state
            assert state['settings']['IslandMaterial'] == material, state['settings']
            assert not state['lastError'], state['lastError']
            if is_music:
                assert state['isPlaying'] and state['title'], state
                return None
            timer = next(t for t in state['timers'] if t['id'] == identifier)
            assert timer['running'] and timer['phase'] == 'focus', timer
            return timer['elapsedMs']

        first = app.request('state')
        initial_timer = validate(first)
        start = time.monotonic()
        cpu, _, _ = sample(app.process)
        rows = []
        for i in range(seconds):
            time.sleep(max(0, start + i + 1 - time.monotonic()))
            rows.append(sample(app.process))
        elapsed = time.monotonic() - start
        last = app.request('state')
        final_timer = validate(last)
        if identifier:
            assert final_timer - initial_timer >= (elapsed - 2) * 1000, (initial_timer, final_timer)
        frames = last['renderFrames'] - first['renderFrames']
        assert frames > 0 and last['rendererGeneration'] == first['rendererGeneration'], last
        shot = app.data / 'expanded.png'
        assert app.request('debugSnapshot', str(shot))['success'] and shot.stat().st_size > 0
        return dict(privateBytes=statistics.median(x[1] for x in rows),
                    workingSetBytes=statistics.median(x[2] for x in rows),
                    maxSamplePrivateBytes=max(x[1] for x in rows),
                    cpuMsPerSecond=(rows[-1][0] - cpu) / elapsed,
                    framesPerSecond=frames / elapsed,
                    glassCaptures=last['glassCaptures'] - first['glassCaptures'],
                    samples=rows, screenshot=str(shot), log=str(app.data / 'process.log'))
    finally:
        app.stop()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--seconds', type=int, default=12)
    parser.add_argument('--runs', type=int, default=2)
    parser.add_argument('--material', nargs='+', default=['Acrylic', 'Glass'])
    cli = parser.parse_args()
    variants = [('baseline', cli.baseline)]
    if cli.candidate:
        variants.append(('candidate', cli.candidate))
    dest = cli.report.resolve()
    dest.parent.mkdir(parents=True, exist_ok=True)
    output = dict(seconds=cli.seconds, runs=cli.runs, workloads={})
    for material in cli.material:
        for scenario in ('timer', 'music'):
            results = output['workloads'].setdefault(f'{scenario}/{material}', {})
            for trial in range(cli.runs):
                for label, runtime in (variants if trial % 2 == 0 else variants[::-1]):
                    result = run(runtime, dest.parent / 'active-data', scenario, material, cli.seconds)
                    results.setdefault(label, []).append(result)
                    print(label, scenario, material, {k: round(v, 2) for k, v in result.items()
                                                       if isinstance(v, (int, float))}, flush=True)
                    dest.write_text(json.dumps(output, indent=2), encoding='utf-8')
