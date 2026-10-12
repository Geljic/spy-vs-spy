#!/usr/bin/env python3
"""Spy vs. Spy (PS2) DATA.PAK reader/extractor.

Reads DATA.PAK straight out of the player's own disc image (ISO/MDF) or a
loose DATA.PAK. Format notes: notes/formats.md#datapak

Usage:
  pak.py list    <image-or-pak>
  pak.py extract <image-or-pak> <out-dir> [substring-filter]
"""
import os
import struct
import sys

SECTOR = 0x800
ENTRY = 48


def open_pak(path):
    """Return a file object positioned so offset 0 is the start of DATA.PAK."""
    if path.lower().endswith('.pak'):
        return open(path, 'rb'), 0
    # Disc image (ISO, or an MDF of a DVD, which is plain 2048-byte sectors)
    f = open(path, 'rb')
    return f, _find_iso_file(f, b'DATA.PAK;1')


def _find_iso_file(f, name):
    f.seek(16 * SECTOR)
    pvd = f.read(SECTOR)
    if pvd[1:6] != b'CD001':
        raise ValueError('not an ISO9660 image (try extracting DATA.PAK first)')
    root = pvd[156:156 + 34]
    lba, size = struct.unpack_from('<I', root, 2)[0], struct.unpack_from('<I', root, 10)[0]
    f.seek(lba * SECTOR)
    d = f.read(size)
    i = 0
    while i < len(d):
        ln = d[i]
        if ln == 0:
            i = (i // SECTOR + 1) * SECTOR
            continue
        nlen = d[i + 32]
        if d[i + 33:i + 33 + nlen] == name:
            return struct.unpack_from('<I', d, i + 2)[0] * SECTOR
        i += ln
    raise FileNotFoundError(name)


def read_toc(f, base):
    f.seek(base)
    count, version = struct.unpack('<II', f.read(8))
    f.seek(base + SECTOR)
    entries = []
    for _ in range(count):
        b = f.read(ENTRY)
        # 40-byte name; bytes after the NUL are uninitialised garbage
        name = b[:40].split(b'\0')[0].decode('ascii')
        sector, size = struct.unpack_from('<II', b, 40)
        entries.append((name, sector, size))
    return version, entries


def main(argv):
    if len(argv) < 3 or argv[1] not in ('list', 'extract'):
        sys.exit(__doc__)
    f, base = open_pak(argv[2])
    _, entries = read_toc(f, base)
    if argv[1] == 'list':
        for name, sector, size in entries:
            print(f'{sector:8} {size:10} {name}')
        return
    out = argv[3]
    filt = argv[4] if len(argv) > 4 else ''
    for name, sector, size in entries:
        if filt not in name or size == 0:
            continue
        dst = os.path.join(out, name)
        os.makedirs(os.path.dirname(dst) or out, exist_ok=True)
        f.seek(base + sector * SECTOR)
        with open(dst, 'wb') as o:
            o.write(f.read(size))
    print(f'extracted to {out}')


if __name__ == '__main__':
    main(sys.argv)
