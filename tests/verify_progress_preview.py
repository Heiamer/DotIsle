"""Exercise all progress styles over the isolated native settings pipe."""
import argparse
import base64
import hashlib
import io
import json
import tempfile
import time
from pathlib import Path

from PIL import Image, ImageDraw
from verify_pomodoro import Runtime

STYLES = ('Normal','Wavy','Rounded','Thin','Thick','Gradient','Segmented','Dots')
THUMBS = ('Default','Circle','Square','Pill','Diamond','Flower','Heart','Cookie','Puffy','Clover','Clover8','Gem','Triangle','Pentagon','Cookie12','Clam')


def run(runtime, scratch, output):
    scratch.mkdir(parents=True, exist_ok=True)
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=scratch, prefix='progress-') as data:
        app = Runtime(runtime, Path(data))
        try:
            app.write_settings()
            app.launch(args=['--demo'])
            def capture(**override):
                payload = dict(width=444, ink=0x2f486b, rail=0xc5cfdd, animate=True, glow=False, thumbOn=False, spin=False)
                payload.update(override)
                response = app.request('playbackProgressPreview', payload)
                assert response.get('success'), response
                blob = base64.b64decode(response['png'], validate=True)
                img = Image.open(io.BytesIO(blob)).convert('RGBA')
                assert img.size == (666, 42) and img.getchannel('A').getbbox() and img.getpixel((0, 0))[3] == 0
                return hashlib.sha256(blob).hexdigest(), img
            styles = {style:capture(bar=style) for style in STYLES}
            assert len({digest for digest, _ in styles.values()}) == len(STYLES), 'two distinct bar styles draw identically'
            thumbs = {style:capture(bar='Normal', thumbOn=True, thumb=style)[0] for style in THUMBS}
            # Validate each offered thumb actually differs from hidden; some designs can share a silhouette.
            assert all(digest != styles['Normal'][0] for digest in thumbs.values())
            assert capture(bar='Wavy', animate=False)[0] != styles['Wavy'][0]
            assert capture(bar='Normal', glow=True)[0] != styles['Normal'][0]
            assert capture(bar='Normal', thumbOn=True, thumb='Diamond', spin=True)[0] != thumbs['Diamond']
            preview = Image.new('RGBA', (770, 50*len(STYLES)+20), (245, 246, 248))
            labels = ImageDraw.Draw(preview)
            for i, (name, (_, img)) in enumerate(styles.items()):
                y = 10+i*50
                labels.text((12, y+15), name, fill=(47, 72, 107))
                preview.alpha_composite(img, (90, y))
            preview.convert('RGB').save(output/'进度条样式.png')
            if app.request('expand').get('success'):
                time.sleep(.85)
                assert app.request('state').get('expanded'), 'island did not expand'
                snapshot=output/'expanded-island.png'
                assert app.request('debugSnapshot', str(snapshot.resolve()))['success']
            report = dict(success=True, bars=list(styles), thumbs=list(thumbs), animatedWave=True, glow=True, spin=True)
            (output/'progress-report.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
            print(json.dumps(report))
        finally:
            app.stop()


if __name__ == '__main__':
    p=argparse.ArgumentParser()
    for name in ('runtime','scratch','output'):
        p.add_argument('--'+name,required=True,type=Path)
    a=p.parse_args()
    run(a.runtime, a.scratch, a.output)
