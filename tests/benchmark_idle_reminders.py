"""Compare production idle CPU/memory without polling the process under test."""
import argparse
import ctypes as c
import json
import statistics
import time
import uuid
from ctypes import wintypes as w
from pathlib import Path

from verify_pomodoro import Runtime


class Memory(c.Structure):
    _fields_ = [('cb', w.DWORD), ('faults', w.DWORD)] + [(key, c.c_size_t) for key in (
        'peakWorkingSet', 'workingSet', 'quotaPeakPaged', 'quotaPaged',
        'quotaPeakNonPaged', 'quotaNonPaged', 'pagefile', 'peakPagefile', 'private')]


def sample(process):
    handle = w.HANDLE(int(process._handle))
    times = (c.c_uint64 * 4)()
    if not c.windll.kernel32.GetProcessTimes(handle, *(c.byref(times, i * 8) for i in range(4))):
        raise c.WinError()
    memory = Memory()
    memory.cb = c.sizeof(memory)
    if not c.windll.psapi.GetProcessMemoryInfo(handle, c.byref(memory), memory.cb):
        raise c.WinError()
    return (times[2] + times[3]) / 10_000, memory.private, memory.workingSet


def bench(runtime, scratch, seconds):
    app = Runtime(runtime, scratch / uuid.uuid4().hex)
    try:
        app.data.mkdir(parents=True)
        app.write_settings()
        path = app.data / 'settings.json'
        settings = json.loads(path.read_text(encoding='utf-8'))
        settings.update(StartupSignatureEnabled=False, StartupGreetingEnabled=False,
                        WeatherAlertsEnabled=False, Animation=False,
                        AllowedSources=['LyricIsland.Verify13'])
        path.write_text(json.dumps(settings), encoding='utf-8')
        app.start(preserve_settings=True)
        time.sleep(7)
        first = app.request('state')
        assert not first['islandVisible'] and not first['isPlaying'] and not first['timers'], first
        start = time.monotonic()
        cpu, _, _ = sample(app.process)
        time.sleep(seconds)
        end_cpu, private, working_set = sample(app.process)
        elapsed = time.monotonic() - start
        last = app.request('state')
        cold = dict(cpuMsPerSecond=(end_cpu - cpu) / elapsed, privateBytes=private,
                    workingSetBytes=working_set, renderedFrames=last['renderFrames'] - first['renderFrames'])
        assert app.request('notificationPreview')['success']
        active = app.request('state')
        assert active['notificationVisible'], active
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            if not app.request('state')['islandVisible']:
                break
            time.sleep(.15)
        else:
            raise AssertionError('notification did not retire')
        time.sleep(24)
        _, private, working_set = sample(app.process)
        idle = app.request('state')
        warmed = dict(privateBytes=private, workingSetBytes=working_set,
                      rendererLoaded=idle.get('rendererLoaded', idle['rendererGeneration'] == active['rendererGeneration']))
        assert not idle['islandVisible'], idle
        assert app.request('notificationPreview')['success']
        resumed = app.request('state')
        assert resumed['notificationVisible'] and resumed['renderFrames'] > idle['renderFrames'], resumed
        return dict(cold=cold, after24SecondsHidden=warmed)
    finally:
        app.stop()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--seconds', type=int, default=16)
    parser.add_argument('--runs', type=int, default=2)
    args = parser.parse_args()
    results = {}
    for trial in range(args.runs):
        order = [('baseline', args.baseline), ('candidate', args.candidate)]
        for variant, runtime in (order if trial % 2 == 0 else reversed(order)):
            result = bench(runtime.resolve(), args.report.resolve().parent / 'perf-data', args.seconds)
            print(variant, result, flush=True)
            results.setdefault(variant, []).append(result)
    median = {variant: {phase: {key: statistics.median(r[phase][key] for r in rows)
                               for key in rows[0][phase] if key != 'rendererLoaded'}
                        for phase in rows[0]} for variant, rows in results.items()}
    args.report.write_text(json.dumps(dict(seconds=args.seconds, runs=args.runs, samples=results, median=median), indent=2), encoding='utf-8')
