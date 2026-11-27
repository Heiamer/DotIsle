"""Reject stale lyric DLLs and incompatible native workers before packaging."""
from pathlib import Path
import ctypes
import json
import queue
import subprocess
import threading

COMMIT = '3fae9287dfb0636fc19a0df2587a1a59a54cf9bb'


def verify(binary_dir):
    binary_dir = Path(binary_dir).resolve()
    class Buffer(ctypes.Structure):
        _fields_ = [('ptr', ctypes.c_void_p), ('len', ctypes.c_size_t)]
    dll = ctypes.CDLL(str(binary_dir / 'dotisle_lyrics.dll'), winmode=0x1100)
    call = dll.dotisle_service_call
    call.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
    call.restype = Buffer
    free = dll.dotisle_service_free
    free.argtypes = [Buffer]
    request = b'{"method":"engine.describe"}'
    result = call(request, len(request))
    try:
        if not result.ptr or result.len > 1024 * 1024:
            raise RuntimeError('Invalid lyric engine identity response')
        reply = json.loads(ctypes.string_at(result.ptr, result.len))
    finally:
        free(result)
    expected = dict(engine='taskbar', commit=COMMIT, worker='native-go')
    if reply.get('ok') is not True or reply.get('value') != expected:
        raise RuntimeError(f'Stale lyric DLL: expected {expected}, received {reply}')
    # Keep stdin open until the worker has replied, exactly like the Rust host.
    process = subprocess.Popen([str(binary_dir / 'LyricIsland.Center.exe'), '--lyrics-worker'],
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               creationflags=subprocess.CREATE_NO_WINDOW)
    line = queue.Queue()
    threading.Thread(target=lambda: line.put(process.stdout.readline()), daemon=True).start()
    try:
        process.stdin.write(json.dumps(dict(ID=1, Version=1, Op='taskbar-plan',
                                           Title='Song', Artist='Artist', TimeoutMS=5000)).encode() + b'\n')
        process.stdin.flush()
        reply = json.loads(line.get(timeout=10))
        if reply.get('ID') != 1 or reply.get('Error') or reply.get('Hits', [{}])[0].get('Extra', {}).get('query') != 'Song Artist':
            raise RuntimeError(f'Incompatible native lyric worker: {reply}')
        threading.Thread(target=lambda: line.put(process.stdout.readline()), daemon=True).start()
        process.stdin.write(b'{"ID":2,"Version":1,"Op":"search","TimeoutMS":1000}\n')
        process.stdin.flush()
        retired = json.loads(line.get(timeout=5))
        if retired.get('ID') != 2 or retired.get('Error') != 'unknown lyric operation':
            raise RuntimeError(f'Retired lyric operation is still enabled: {retired}')
    finally:
        process.terminate()
        process.communicate(timeout=5)
    return dict(success=True, **expected)


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('binary_dir', type=Path)
    print(json.dumps(verify(parser.parse_args().binary_dir)))
