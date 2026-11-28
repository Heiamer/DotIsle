"""Exercise per-relay Agent start with actual process scans and an isolated HTTP endpoint."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import shutil
import subprocess
import threading
import time
import uuid

from verify_pomodoro import Runtime
from verify_relay_balance import seal


def run(runtime, fixture, report):
    root = report.parent / ('relay-agent-' + uuid.uuid4().hex)
    root.mkdir(parents=True)
    canary = 'private-relay-agent-fixture'
    count = [0]
    app = Runtime(runtime, root / 'data')
    children = []

    class Relay(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_GET(self):
            if self.path != '/v1/usage' or self.headers.get('Authorization') != 'Bearer ' + canary:
                self.send_error(403)
                return
            count[0] += 1
            body = b'{"isValid":true,"mode":"unrestricted","balance":4.2,"unit":"USD"}'
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    def launch_agent(name):
        exe = root / (name + '.exe')
        if not exe.exists():
            shutil.copy2(fixture, exe)
        proc = subprocess.Popen([str(exe)], creationflags=subprocess.CREATE_NO_WINDOW)
        children.append(proc)
        return proc

    def stop_agent(proc):
        if proc.poll() is None:
            proc.terminate()
            proc.wait(timeout=5)

    def wait(predicate, timeout=12):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            state = app.request('state')
            if predicate(state['aiQuota']):
                return state['aiQuota']
            time.sleep(.1)
        raise AssertionError(f'Relay Agent check timed out, calls={count[0]}')

    with ThreadingHTTPServer(('127.0.0.1', 0), Relay) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            site = f'http://127.0.0.1:{server.server_port}'
            identifier = 'ace00ace00ace00ace00ace00ace00aa'
            app.data.mkdir()
            app.write_settings()
            path = app.data / 'settings.json'
            settings = json.loads(path.read_text(encoding='utf-8'))
            settings.update(AiQuotaMonitor=dict(enabled=True, fiveHour=20, weekly=10),
                            AiQuotaSources=[dict(id=identifier, provider='sub2api', site=site, name='Agent Relay Fixture', agent='cursor')])
            path.write_text(json.dumps(settings), encoding='utf-8')
            seal(app.data/'ai-quota'/f'{identifier}.dpapi', dict(site=site, provider='sub2api', token=canary))
            app.start(preserve_settings=True)
            row = lambda q: next((r for r in q['readings'] if r['id'] == identifier), {})
            wait(lambda q: not q['pending'] and row(q).get('status') == '等待 Cursor 启动')
            time.sleep(2.3)
            assert count[0] == 0, 'Background collection contacted an inactive bound relay'
            wrong = launch_agent('Claude')
            time.sleep(3.0)
            assert wrong.poll() is None and count[0] == 0, 'An unrelated Agent triggered the relay'
            stop_agent(wrong)
            correct = launch_agent('Cursor')
            wait(lambda q: count[0] == 1 and not q['pending'] and bool(row(q).get('metrics')))
            time.sleep(3.1)
            assert count[0] == 1, 'A continuously running Agent repeatedly queried a startup-only relay'
            stop_agent(correct)
            time.sleep(2.6)
            correct = launch_agent('Cursor')
            wait(lambda q: count[0] == 2 and not q['pending'])
            stop_agent(correct)
            assert not app.request('aiQuotaSourceTrigger', {'id': identifier, 'agent': 'antigravity'})['success']
            assert app.request('aiQuotaSourceTrigger', {'id': identifier, 'agent': 'claude'})['success']
            wait(lambda q: not q['pending'] and row(q).get('status') == '等待 Claude 启动')
            time.sleep(2.5)
            assert count[0] == 2, 'Changing the binding queried a stopped Agent'
            assert app.request('aiQuotaRefresh')['success']
            info = wait(lambda q: count[0] == 3 and not q['pending'] and bool(row(q).get('metrics')))
            assert info['profiles'][0]['agent'] == 'claude' and canary not in json.dumps(info)
            assert app.request('prepareUpdateExit')['success']
            app.process.wait(timeout=10)
            app.stop()
            state = app.start(preserve_settings=True)
            assert state['aiQuota']['profiles'][0]['agent'] == 'claude'
            wait(lambda q: not q['pending'] and row(q).get('status') == '等待 Claude 启动')
            assert count[0] == 3
            report.write_text(json.dumps(dict(success=True, relayCalls=count[0], checks=[
                'No requests while the configured Agent is stopped, even when other Agents run',
                'Correct Agent starts and restarts each trigger exactly one real balance request',
                'Existing binding can be changed without re-entering credentials',
                'Manual refresh bypasses the trigger and binding persists through restart',
            ]), indent=2), encoding='utf-8')
        finally:
            for proc in children:
                stop_agent(proc)
            app.stop()
            server.shutdown()
            thread.join(timeout=3)


if __name__ == '__main__':
    cli = argparse.ArgumentParser()
    cli.add_argument('--runtime', type=Path, required=True)
    cli.add_argument('--fixture', type=Path, required=True)
    cli.add_argument('--report', type=Path, required=True)
    a = cli.parse_args()
    run(a.runtime.resolve(), a.fixture.resolve(), a.report.resolve())
