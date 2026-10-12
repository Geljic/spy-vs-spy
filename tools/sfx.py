#!/usr/bin/env python3
"""Extract sound effects (PS-ADPCM) from a container file to WAV.

Format notes: notes/formats.md#sound

Usage: sfx.py <file> <out-dir>
"""
import os
import struct
import sys
import wave

from container import Container

RATES = (44100, 22050, 11025, 48000, 32000, 24000, 16000)
COEF = ((0, 0), (60, 0), (115, -52), (98, -55), (122, -60))


def valid_frame(f):
    return (f[0] >> 4) <= 4 and (f[0] & 0xF) <= 12 and f[1] in (0, 1, 2, 3, 4, 6, 7)


def find_frames(d, o, size):
    """Locate the ADPCM frame run: find the end frame, then walk back."""
    end = o + size
    # end frame: flags 7 followed by 0x77 padding, or flags with bit 0 set
    j = d.rfind(b'\x07\x77\x77\x77', o, end)
    if j < 0:
        return None
    last = j - 1                      # start of the end frame
    start = last
    while start - 16 >= o + 28 and valid_frame(d[start - 16:start]):
        start -= 16
    return start, last + 16


def decode(adpcm):
    out = []
    h1 = h2 = 0
    for k in range(0, len(adpcm) - 15, 16):
        f = adpcm[k:k + 16]
        shift, filt = f[0] & 0xF, min(f[0] >> 4, 4)
        c1, c2 = COEF[filt]
        for b in f[2:]:
            for nib in (b & 0xF, b >> 4):
                s = (nib << 12) & 0xFFFF
                if s & 0x8000:
                    s -= 0x10000
                s = (s >> shift) + ((h1 * c1 + h2 * c2 + 32) >> 6)
                s = max(-32768, min(32767, s))
                h2, h1 = h1, s
                out.append(s)
        if f[1] & 1:
            break
    return out


def main(argv):
    if len(argv) != 3:
        sys.exit(__doc__)
    c = Container.load(argv[1])
    d = c.data
    os.makedirs(argv[2], exist_ok=True)
    n = 0
    for o, size, _ in c.allocations():
        if size < 64 or d[o + 8:o + 12] != b'\x10\x00\x00\x00':
            continue
        rate = struct.unpack_from('<I', d, o + 12)[0]
        if rate not in RATES:
            continue
        span = find_frames(d, o, size)
        if not span:
            continue
        pcm = decode(d[span[0]:span[1]])
        name = c.name_near(c.refs[o][0]) or f'sfx_{o:08x}'
        with wave.open(os.path.join(argv[2], f'{name}.wav'), 'wb') as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(rate)
            w.writeframes(struct.pack(f'<{len(pcm)}h', *pcm))
        n += 1
    print(f'{n} sounds -> {argv[2]}')


if __name__ == '__main__':
    main(sys.argv)
