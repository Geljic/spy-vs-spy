#!/usr/bin/env python3
"""Extract textures from a container file (.map, .ls, ...) to PNG.

Format notes: notes/formats.md#texture

Usage: tex.py <file> <out-dir>
"""
import os
import struct
import sys

from PIL import Image

from container import Container, PTR

HEADER = 27      # guard(4) + flags(4) + pointer(12) + fmt/w/h/mips/pad(7)
PALETTE = 1024   # 256 x RGBA, alpha 0..0x80


def csm1(i):
    """PS2 CSM1 palette order: swap bits 3 and 4 of the index."""
    return (i & 0xE7) | ((i & 0x08) << 1) | ((i & 0x10) >> 1)


def unswizzle8(buf, w, h):
    """Undo GS PSMT8 block swizzling (pixels stored as if PSMT32)."""
    out = bytearray(w * h)
    for y in range(h):
        for x in range(w):
            block = (y & ~0xF) * w + (x & ~0xF) * 2
            swap = (((y + 2) >> 2) & 1) * 4
            py = (((y & ~3) >> 1) + (y & 1)) & 7
            col = py * w * 2 + ((x + swap) & 7) * 4
            byte = ((y >> 1) & 1) + ((x >> 2) & 2)
            out[y * w + x] = buf[block + col + byte]
    return out


def parse(d, o, size):
    """Return (w, h, mips, pixels, palette) if the allocation is a texture."""
    if size < HEADER + PALETTE or d[o + 8:o + 12] != PTR:
        return None
    fmt, w, h, mips = struct.unpack_from('<BHHB', d, o + 20)
    if fmt != 1 or not (8 <= w <= 1024 and 8 <= h <= 1024) or not 1 <= mips <= 8:
        return None
    npix = sum((w >> m) * (h >> m) for m in range(mips))
    if HEADER + npix + PALETTE + 1 != size:
        return None
    pix = d[o + HEADER:o + HEADER + w * h]
    pal = d[o + HEADER + npix:o + HEADER + npix + PALETTE]
    return w, h, mips, pix, pal


def to_image(w, h, pix, pal):
    colors = []
    for i in range(256):
        j = 4 * csm1(i)
        colors.append((pal[j], pal[j + 1], pal[j + 2], min(255, pal[j + 3] * 2)))
    if w >= 16 and h >= 4:
        pix = unswizzle8(pix, w, h)
    im = Image.new('RGBA', (w, h))
    im.putdata([colors[b] for b in pix])
    return im


def main(argv):
    if len(argv) != 3:
        sys.exit(__doc__)
    c = Container.load(argv[1])
    os.makedirs(argv[2], exist_ok=True)
    seen = {}
    n = 0
    for o, size, _ in c.allocations():
        t = parse(c.data, o, size)
        if not t:
            continue
        w, h, mips, pix, pal = t
        name = c.name_near(c.refs[o][0]) or f'tex_{o:08x}'
        seen[name] = seen.get(name, 0) + 1
        if seen[name] > 1:
            name = f'{name}_{seen[name]}'
        to_image(w, h, pix, pal).save(os.path.join(argv[2], f'{name}.png'))
        n += 1
    print(f'{n} textures -> {argv[2]}')


if __name__ == '__main__':
    main(sys.argv)
