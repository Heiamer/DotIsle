"""Validate production-renderer PNGs from --resident-preview (requires Pillow)."""
import argparse
import json
from pathlib import Path
from PIL import Image


def verify(directory):
    report = json.loads((directory / 'report.json').read_text(encoding='utf-8'))
    visible = 0
    for case in report['cases']:
        with Image.open(directory / case['file']) as image:
            bounds = image.getchannel('A').getbbox()
        assert bounds is not None, f"Resident island disappeared: {case['file']}"
        visible += 1
        if case['task'] in ('expanded', 'expanded-empty'):
            assert case['deviceRows'] == (2 if case['task'] == 'expanded' else 0), case
        assert case['activeComponent'] == ('music' if case['task'] == 'music' else 'resident'), case
    assert visible == 108
    return dict(success=True, visibleCases=visible)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    result = verify(args.directory)
    args.report.write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result))
