"""Validate production-renderer PNGs from --resident-preview (requires Pillow)."""
import argparse
import json
from pathlib import Path
from PIL import Image


def verify(directory):
    report = json.loads((directory / 'report.json').read_text(encoding='utf-8'))
    transparent = visible = 0
    for case in report['cases']:
        with Image.open(directory / case['file']) as image:
            bounds = image.getchannel('A').getbbox()
        if case['task'] in ('idle', 'paused', 'returned'):
            assert bounds is None, f"Idle pixels remain: {case['file']}"
            transparent += 1
        else:
            assert bounds is not None, f"Occupied island disappeared: {case['file']}"
            visible += 1
    assert transparent == 36 and visible == 24
    return dict(success=True, fullyTransparentCases=transparent, visibleTaskCases=visible,
                totalCases=transparent + visible)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    result = verify(args.directory)
    args.report.write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result))
