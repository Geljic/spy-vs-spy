#!/usr/bin/env python3
"""Reference views: render our build from a fixed camera and compare with a frame of the original.

Writes work/ref_<name>.png (original | ours) and prints a rough score (0-100, higher = closer):
colour similarity of 16x9 downscales, ignoring the HUD corners. Track it over time; it's a trend, not truth.
Usage: tests/ref_compare.py [name...]
"""
import os
import subprocess
import sys

from PIL import Image, ImageChops

HERE = os.path.dirname(os.path.abspath(__file__))
WORK = os.path.abspath(os.path.join(HERE, '..', '..', 'work'))
BIN = os.path.join(HERE, '..', 'target', 'debug', 'spy-vs-spy')


def score(a, b):
    a = a.convert('RGB').resize((16, 9), Image.BILINEAR)
    b = b.convert('RGB').resize((16, 9), Image.BILINEAR)
    diff = ImageChops.difference(a, b)
    px = [p for i, p in enumerate(diff.getdata()) if not (i // 16 < 2 and (i % 16 < 4 or i % 16 > 11))]
    mean = sum(sum(p) for p in px) / (3 * len(px))
    return max(0.0, 100 - mean / 2.55)


def main(only):
    for line in open(os.path.join(HERE, 'ref', 'views.txt')):
        line = line.split('#')[0].strip()
        if not line:
            continue
        name, mp, cam, clip, t = [x.strip() for x in line.split('|')]
        if only and name not in only:
            continue
        ref = os.path.join(WORK, f'ref_{name}_orig.png')
        subprocess.run(['ffmpeg', '-v', 'error', '-y', '-ss', t, '-i', clip, '-frames:v', '1', '-vf', 'scale=960:540', ref], check=True)
        env = dict(os.environ, SVS_MAP=mp, SVS_CAM=cam, SVS_ARENA_TEST='1', SVS_SHOT_DIR=WORK)
        subprocess.run([BIN], env=env, cwd=os.path.dirname(BIN), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=180)
        ours = Image.open(os.path.join(WORK, f'arena_{mp}.png')).convert('RGB')
        ours = ours.crop((0, 0, ours.width // 2, ours.height)).resize((480, 540))
        orig = Image.open(ref).convert('RGB')
        # the original is 16:9 full-screen; compare against its centre half (same framing as our half view)
        orig_c = orig.crop((240, 0, 720, 540))
        sheet = Image.new('RGB', (960 + 480, 540))
        sheet.paste(orig, (0, 0))
        sheet.paste(ours, (960, 0))
        out = os.path.join(WORK, f'ref_{name}.png')
        sheet.save(out)
        print(f'{name:16} score {score(orig_c, ours):5.1f}  -> {out}')


if __name__ == '__main__':
    main(sys.argv[1:])
