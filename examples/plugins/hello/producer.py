"""Public plugin example; standard Python only, no network or host commands."""
import json
import os
from pathlib import Path
import time

folder = Path(__file__).resolve().parent
started = time.monotonic()
try:
    while True:
        elapsed = int(time.monotonic() - started)
        temporary = folder / 'state.pending.json'
        temporary.write_text(json.dumps({'counter': elapsed, 'progress': elapsed % 60 / 60}), encoding='utf-8')
        os.replace(temporary, folder / 'state.json')
        time.sleep(1)
except KeyboardInterrupt:
    pass
