"""Verify the shipped native USD and CNY balance notifications against a private relay fixture."""
import argparse
import ctypes as c
from ctypes import wintypes as w
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import threading
import time
import uuid

from verify_pomodoro import Runtime


class Blob(c.Structure):
    _fields_ = [('size', w.DWORD), ('content', c.POINTER(c.c_ubyte))]


def seal(path, value):
    raw = json.dumps(value).encode()
    clear = (c.c_ubyte * len(raw)).from_buffer_copy(raw)
    inp, out = Blob(len(clear), clear), Blob()
    crypt = c.WinDLL('crypt32', use_last_error=True)
    free = c.WinDLL('kernel32').LocalFree
    free.argtypes = [c.c_void_p]
    crypt.CryptProtectData.argtypes = [c.POINTER(Blob), w.LPCWSTR, c.c_void_p,
                                      c.c_void_p, c.c_void_p, w.DWORD, c.POINTER(Blob)]
    if not crypt.CryptProtectData(c.byref(inp), None, None, None, None, 1, c.byref(out)):
        raise c.WinError(c.get_last_error())
    try:
        path.parent.mkdir(parents=True)
        path.write_bytes(c.string_at(out.content, out.size))
    finally:
        free(out.content)
        c.memset(clear, 0, len(clear))


def run(runtime, report):
    app = Runtime(runtime, report.parent / ('balance-' + uuid.uuid4().hex))
    account = {'unit': 'USD', 'amount': 1., 'calls': 0}
    token = 'fixture-only-in-notebook'

    class Relay(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_GET(self):
            if self.path != '/v1/usage' or self.headers.get('Authorization') != 'Bearer '+token:
                self.send_error(403)
                return
            account['calls'] += 1
            body = json.dumps(dict(isValid=True, mode='unrestricted', balance=account['amount'], unit=account['unit'])).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    with ThreadingHTTPServer(('127.0.0.1', 0), Relay) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        site = f'http://127.0.0.1:{server.server_port}'
        id = uuid.uuid4().hex
        key = lambda unit: 'quota:balance:' + id + ':' + unit
        def wait(pred, timeout=12):
            deadline = time.monotonic()+timeout
            state = {}
            while time.monotonic() < deadline:
                state = app.request('state')
                if pred(state):
                    return state
                time.sleep(.10)
            raise AssertionError((account, state.get('aiQuota'), state.get('testAlertKey')))
        def refresh(unit, amount):
            current = account['calls']
            account.update(unit=unit, amount=amount)
            reply = app.request('aiQuotaRefresh')
            assert reply['success'], reply
            return wait(lambda s: account['calls'] > current and not s['aiQuota']['pending'])
        try:
            app.data.mkdir(parents=True)
            app.write_settings()
            path = app.data / 'settings.json'
            settings = json.loads(path.read_text(encoding='utf-8'))
            settings.update(StartupSignatureEnabled=False, StartupGreetingEnabled=False,
                            WeatherAlertsEnabled=False, Animation=False,
                            AllowedSources=['LyricIsland.Verify13'],
                            AiQuotaSources=[dict(id=id, provider='sub2api', site=site, name='Local Balance Fixture')])
            path.write_text(json.dumps(settings), encoding='utf-8')
            seal(app.data/'ai-quota'/f'{id}.dpapi', dict(site=site, provider='sub2api', token=token))
            state = app.start(preserve_settings=True)
            quota = lambda s: s['aiQuota']['settings']
            balance_alert = lambda s: str(s.get('testAlertKey', '')).startswith('quota:balance:')
            assert not quota(state)['enabled'] and quota(state)['relayCurrency'] == 'USD' and account['calls'] == 0
            def configure(enabled, currency=None):
                requested = dict(enabled=enabled, fiveHour=20, weekly=10)
                if currency is not None:
                    requested['relayCurrency'] = currency
                return app.request('aiQuotaConfigure', requested)

            assert not configure(True, 'EUR')['success']
            assert configure(True)['success']
            exact = wait(lambda s: account['calls'] > 0 and not s['aiQuota']['pending'])
            assert not balance_alert(exact)
            for unit in ('CNY', None, '积分', 'TOKENS'):
                sample = refresh(unit, .47)
                assert not balance_alert(sample)
                values = [m['value'] for r in sample['aiQuota']['readings'] if r['id'] == id for m in r['metrics']]
                assert values
                if unit in (None, '积分', 'TOKENS'):
                    label = 'Tokens' if unit == 'TOKENS' else (unit or '单位未知')
                    assert any('0.47' in value and label in value and '$' not in value and '¥' not in value for value in values)
            refresh('USD', .72)
            low = wait(lambda s: s.get('testAlertKey') == key('USD') and s['islandVisible'])
            assert low['testAlertRect']['h'] == 112
            wait(lambda s: not s['capsuleAlertVisible'], 14)
            assert not balance_alert(refresh('USD', 0))

            # The same source can alert in CNY after switching, without USD/UI dedup collisions.
            assert configure(True, 'CNY')['success']
            wait(lambda s: quota(s)['relayCurrency'] == 'CNY')
            assert not balance_alert(refresh('CNY', 1))
            for unit in ('USD', None, '积分', 'TOKENS'):
                assert not balance_alert(refresh(unit, .47))
            refresh('CNY', .68)
            low = wait(lambda s: s.get('testAlertKey') == key('CNY') and s['islandVisible'])
            assert low['testAlertRect']['h'] == 112
            wait(lambda s: not s['capsuleAlertVisible'], 14)
            assert not balance_alert(refresh('RMB', 0))

            assert token not in json.dumps(app.request('state'))
            assert configure(False)['success'], 'legacy client can turn off without clearing CNY'
            before = account['calls']
            assert app.request('aiQuotaRefresh')['success']
            time.sleep(.6)
            assert account['calls'] == before
            assert app.request('prepareUpdateExit')['success']
            app.process.wait(timeout=10)
            app.stop()
            state = app.start(preserve_settings=True)
            assert quota(state) == dict(enabled=False, fiveHour=20, weekly=10, relayCurrency='CNY')
            assert account['calls'] == before
            report.write_text(json.dumps(dict(success=True, checks=[
                'Plugin off never reads the relay', 'USD is the default and CNY persists through restart',
                'Both units require strict less-than 1; opposite and unknown units, points and tokens are silent',
                'USD 0.72 and CNY 0.68 each automatically open the native island',
                'Points and unknown units remain visible without a currency symbol',
                'Repeated low balances are silent and secrets do not appear in state'], relayCalls=before), indent=2), encoding='utf-8')
        finally:
            app.stop()
            server.shutdown()
            thread.join(timeout=3)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    options = parser.parse_args()
    run(options.runtime.resolve(), options.report.resolve())
