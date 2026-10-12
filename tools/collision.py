#!/usr/bin/env python3
"""Export a map's collision geometry (the level's physical shape) to Wavefront OBJ.

Format notes: notes/formats.md#collision

Usage: collision.py <file.map> <out.obj>
"""
import struct
import sys

BB = b'\xbb' * 4
BE = b'\xbe' * 4
SCALE = 0.01  # int16 units -> engine units (provisional)


def chunk_at(d, p):
    """(end, version) if a chunk header starts at p."""
    if p % 4 or d[p:p + 4] != BB:
        return None
    end, ver = struct.unpack_from('<II', d, p + 4)
    if end <= p or end + 4 > len(d) or d[end:end + 4] != BE:
        return None
    return end, ver


def parse_collision(d, p):
    """Collision object chunk (v4): base v1 chunk, u8 present, node tree, leaves, polys."""
    end, ver = chunk_at(d, p)
    base = chunk_at(d, p + 12)
    if not base or base[1] != 1:
        return None
    q = base[0] + 4
    if not d[q]:
        return None
    q += 1
    _f14, _f18, n_nodes = struct.unpack_from('<III', d, q)
    q += 12 + n_nodes * 28           # nodes: u16, u16, float3 min, float3 max
    n_leaves = struct.unpack_from('<I', d, q)[0]
    q += 4
    leaves = []
    for _ in range(n_leaves):
        n_polys = d[q + 1]
        w1 = struct.unpack_from('<I', d, q + 2)[0]
        leaves.append((n_polys, (w1 & 0xF) + 1))   # polys, normal count
        q += 26                      # u8, u8, u32, u32, u32, int16x3 lo, int16x3 hi
    q += 4                           # total poly-data size
    tris = []                        # (v0, v1, v2, surface)
    for n_polys, n_normals in leaves:
        descs = []
        for _ in range(n_polys):
            descs.append(struct.unpack_from('<5H' if ver > 3 else '<4H', d, q))
            q += 10 if ver > 3 else 8
        n_verts = max((x[-1] & 0xF for x in descs), default=-1) + 1
        q += 6 * n_normals           # int16x3 unit normals (x16384)
        verts = [struct.unpack_from('<3h', d, q + 6 * k) for k in range(n_verts)]
        q += 6 * n_verts
        # strip: a descriptor with bit 0 set closes a triangle with the two before it
        for k in range(2, len(descs)):
            if descs[k][0] & 1:
                ids = [descs[k - 2][-1] & 0xF, descs[k - 1][-1] & 0xF, descs[k][-1] & 0xF]
                tri = [verts[i] for i in ids]
                if descs[k][1] & 1:
                    tri = [tri[0], tri[2], tri[1]]
                tris.append((tri, descs[k][2]))
    if ((q + 3) & ~3) != end:
        return None
    return tris


def collision_objects(d):
    """Every collision object chunk in a container file."""
    for i in range(0, len(d) - 28, 4):
        if d[i:i + 4] != BB:
            continue
        c = chunk_at(d, i)
        if not c or c[1] != 4:
            continue
        k = chunk_at(d, i + 12)
        if not k or k[1] != 1 or k[0] - (i + 12) != 16:
            continue
        try:
            tris = parse_collision(d, i)
        except (struct.error, IndexError):
            tris = None
        if tris is not None:
            yield i, tris


def main(argv):
    if len(argv) != 3:
        sys.exit(__doc__)
    d = open(argv[1], 'rb').read()
    n_obj = n_tri = 0
    with open(argv[2], 'w') as f:
        f.write('# collision geometry extracted by spy-vs-spy/tools/collision.py\n')
        v = 1
        for at, tris in collision_objects(d):
            n_obj += 1
            f.write(f'o coll_{at:08x}\n')
            for tri, _surface in tris:
                for x, y, z in tri:
                    # file is z-up; OBJ convention is y-up
                    f.write(f'v {x * SCALE:.3f} {z * SCALE:.3f} {-y * SCALE:.3f}\n')
                f.write(f'f {v} {v + 1} {v + 2}\n')
                v += 3
                n_tri += 1
    print(f'{n_obj} objects, {n_tri} triangles -> {argv[2]}')


if __name__ == '__main__':
    main(sys.argv)
