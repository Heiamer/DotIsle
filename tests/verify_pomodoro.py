"""Exercise the packaged native timer over its real, isolated settings pipe."""
import argparse
import ctypes
from ctypes import wintypes
import json
import msvcrt
import os
from pathlib import Path
import subprocess
import time
import uuid

kernel = ctypes.WinDLL('kernel32', use_last_error=True)
kernel.WaitNamedPipeW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD]
kernel.WaitNamedPipeW.restype = wintypes.BOOL
kernel.PeekNamedPipe.argtypes = [wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD,
                                ctypes.c_void_p, ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p]
kernel.PeekNamedPipe.restype = wintypes.BOOL
native = ctypes.WinDLL('ntdll')
native.NtSuspendProcess.argtypes = [wintypes.HANDLE]
native.NtSuspendProcess.restype = ctypes.c_long
native.NtResumeProcess.argtypes = [wintypes.HANDLE]
native.NtResumeProcess.restype = ctypes.c_long


class Runtime:
    def __init__(self, runtime, data):
        self.runtime, self.data = runtime, data
        self.pipe = r'\\.\pipe\LyricIsland.Settings.test.pomodoro.' + uuid.uuid4().hex
        self.process = None
        self.log = None

    def start(self):
        self.data.mkdir(parents=True, exist_ok=True)
        (self.data / 'settings.json').write_text(json.dumps({
            'AutoStart': False, 'AutoCheckUpdatesEnabled': False,
            'DeviceRemindersEnabled': False, 'SystemStateHintsEnabled': False,
            'NotificationsEnabled': False, 'TodoSoundEnabled': False,
            'NotificationSoundEnabled': False, 'DeviceSoundEnabled': False,
            'GestureTutorialSeen': True, 'OnlineSongInfo': False,
        }), encoding='utf-8')
        environment = dict(os.environ, LYRICISLAND_TEST_PIPE=self.pipe.split('\\')[-1])
        self.log = (self.data / 'process.log').open('ab')
        self.process = subprocess.Popen(
            [str(self.runtime / 'LyricIsland.exe'), '--data-dir', str(self.data), '--startup'],
            cwd=self.runtime, env=environment, stdout=self.log, stderr=self.log,
            creationflags=subprocess.CREATE_NO_WINDOW)
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise RuntimeError(f'Native process exited: {self.process.returncode}')
            try:
                return self.request('state')
            except (OSError, TimeoutError):
                time.sleep(.1)
        raise TimeoutError('Native test pipe did not start')

    def request(self, command, value=None):
        deadline = time.monotonic() + 4
        while True:
            if kernel.WaitNamedPipeW(self.pipe, 100):
                try:
                    stream = open(self.pipe, 'r+b', buffering=0)
                    break
                except OSError:
                    pass
            if time.monotonic() >= deadline:
                raise TimeoutError('Cannot connect to test pipe')
            time.sleep(.01)
        with stream:
            # Retry only connection establishment; never replay a mutation.
            packet = json.dumps({'version': 1, 'command': command, 'value': value}) + '\n'
            stream.write(packet.encode('utf-8'))
            response = bytearray()
            deadline = time.monotonic() + 8
            while time.monotonic() < deadline:
                available = wintypes.DWORD()
                if not kernel.PeekNamedPipe(msvcrt.get_osfhandle(stream.fileno()), None, 0,
                                            None, ctypes.byref(available), None):
                    raise ctypes.WinError(ctypes.get_last_error())
                if available.value:
                    response.extend(stream.read(min(available.value, 65536)))
                    if b'\n' in response:
                        return json.loads(response)
                time.sleep(.005)
            raise TimeoutError(f'Pipe response timed out: {command}')

    def create(self, value):
        reply = self.request('timerCreate', value)
        assert reply['success'], reply
        return reply['id']

    def timer(self, identifier):
        return next(t for t in self.request('state')['timers'] if t['id'] == identifier)

    def act(self, identifier, action, revision=None):
        if revision is None:
            revision = self.timer(identifier)['revision']
        return self.request('timerAction', {'id': identifier, 'action': action, 'revision': revision})

    def stall(self, seconds):
        # Suspend only this test's child, simulating a delayed native update.
        assert native.NtSuspendProcess(int(self.process._handle)) == 0
        try:
            time.sleep(seconds)
        finally:
            assert native.NtResumeProcess(int(self.process._handle)) == 0

    def stop(self):
        if self.process is not None and self.process.poll() is None:
            self.process.terminate()
            self.process.wait(timeout=10)
        if self.log is not None:
            self.log.close()


def wait_timer(app, identifier, predicate, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        timer = app.timer(identifier)
        if predicate(timer):
            return timer
        time.sleep(.06)
    raise AssertionError(f'Timer state timed out: {timer}')


def run(args):
    report = {'success': False, 'baseline': args.baseline, 'checks': [], 'snapshots': {}}
    app = Runtime(args.runtime.resolve(), args.scratch.resolve())
    try:
        app.start()
        pomodoro = app.create({'mode': 'pomodoro', 'focus': 5, 'rest': 2, 'rounds': 3})
        initial = app.timer(pomodoro)
        report['snapshots']['initial'] = initial
        assert initial['phase'] == 'focus' and initial['round'] == 1
        app.stall(10.2)
        after = wait_timer(app, pomodoro, lambda t: t['revision'] > initial['revision'])
        report['snapshots']['afterDelayedTick'] = after
        skipped = after['phase'] != 'rest' or after['round'] != 1 or after['finished']
        if args.baseline:
            assert skipped, f'Expected the published bug, observed {after}'
            report['checks'].append('Published 3.2.6 reproduced phase skipping after a 10.2-second stall')
        else:
            assert not skipped, after
            assert after['elapsedMs'] < 1000, after
            sequence = [(1, 'focus'), (after['round'], after['phase'])]
            deadline = time.monotonic() + 25
            while time.monotonic() < deadline:
                current = app.timer(pomodoro)
                phase = (current['round'], current['phase'])
                if sequence[-1] != phase:
                    sequence.append(phase)
                if current['finished']:
                    break
                time.sleep(.06)
            assert current['finished'] and not current['running'], current
            assert sequence == [(1, 'focus'), (1, 'rest'), (2, 'focus'), (2, 'rest'), (3, 'focus')], sequence
            report['snapshots']['sequenceAfterStall'] = sequence
            report['checks'].append('Delayed tick starts a full rest; every remaining focus/rest phase runs in order')
            print('PASS: delayed tick and all three rounds', flush=True)

            forward = app.create({'mode': 'pomodoro', 'focus': 1, 'rest': 1, 'rounds': 2, 'countUp': True})
            wait_timer(app, forward, lambda t: t['phase'] == 'rest')
            wait_timer(app, forward, lambda t: t['phase'] == 'focus' and t['round'] == 2)
            wait_timer(app, forward, lambda t: t['finished'])
            report['checks'].append('Count-up mode also preserves the rest phase and final completion')

            flow = app.create({'mode': 'workflow', 'name': '学习休息回归', 'stages': [
                {'name': '学习', 'seconds': 60}, {'name': '休息', 'seconds': 60},
                {'name': '学习', 'seconds': 60}]})
            revision = app.timer(flow)['revision']
            assert app.act(flow, 'skip', revision)['success']
            assert not app.act(flow, 'skip', revision)['success']
            rest = app.timer(flow)
            assert rest['workflow']['stageIndex'] == 1 and rest['elapsedMs'] < 1000, rest
            assert not app.act(flow, 'next')['success']
            assert app.act(flow, 'pause')['success']
            paused = app.timer(flow)
            time.sleep(.5)
            assert app.timer(flow)['elapsedMs'] == paused['elapsedMs']
            assert app.act(flow, 'resume')['success']
            time.sleep(.4)
            assert app.act(flow, 'pause')['success']
            saved = app.timer(flow)
            assert saved['workflow']['stageIndex'] == 1
            report['snapshots']['workflowSaved'] = saved
            report['snapshots']['timersFileBeforeRestart'] = json.loads((app.data / 'timers.json').read_text(encoding='utf-8'))
            report['checks'].append('Duplicate skip rejected; rest retained; next cannot bypass a running stage; pause/resume stable')
            print('PASS: workflow duplicate actions and pause/resume', flush=True)

            app.stop()
            time.sleep(.15)
            app.start()
            restored = app.timer(flow)
            report['snapshots']['workflowRestored'] = restored
            report['snapshots']['timersFileAfterRestart'] = json.loads((app.data / 'timers.json').read_text(encoding='utf-8'))
            assert not restored['running'] and restored['workflow']['stageIndex'] == 1, restored
            assert restored['elapsedMs'] == saved['elapsedMs'], {'saved': saved, 'restored': restored}
            assert app.act(flow, 'resume')['success']
            assert app.act(flow, 'skip')['success']
            assert app.timer(flow)['workflow']['stageIndex'] == 2
            assert app.act(flow, 'skip')['success']
            assert app.timer(flow)['finished']
            report['checks'].append('Restart retains the same paused rest and elapsed time; final stage finishes correctly')
            print('PASS: persisted rest and restart', flush=True)
        report['success'] = True
    except Exception as error:
        report['error'] = repr(error)
        raise
    finally:
        app.stop()
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('runtime', type=Path)
    parser.add_argument('--scratch', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--baseline', action='store_true')
    run(parser.parse_args())
