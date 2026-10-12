#!/usr/bin/env python3
"""Export a map's textured render meshes to OBJ + MTL + PNG textures (opens in Blender).

Format notes: notes/formats.md#render-mesh

Usage: levelmesh.py <file.map> <out-dir>
"""
import os
import struct
import sys

import tex
from collision import BB, BE, chunk_at
from container import Container

UNIT = 0.006   # int16 units -> metres (same as collision)


def a16(n):
    return (n + 15) & ~15


def children(d, p, end):
    out, q = [], p + 12
    while q < end:
        q = (q + 3) & ~3
        c = chunk_at(d, q)
        if c:
            out.append((q, c[0], c[1]))
            q = c[0] + 4
        else:
            q += 4
    return out


def chunk_name(d, p, end):
    j = d.find(b'\x14\x00\x00\x00', p, min(end, p + 120))
    if j < 0:
        return None
    raw = d[j + 4:j + 24].split(b'\0')[0]
    return raw.decode('latin1') if raw and all(32 < c < 127 for c in raw) else None


def parse_render(d, p):
    end, _ = chunk_at(d, p)
    q = p + 12
    if not d[q]:
        return None
    q += 1
    blobs = []
    for _ in range(4):
        n = struct.unpack_from('<I', d, q)[0]
        blobs.append(q + 4)
        q += 4 + n
    groups = []
    n_groups = struct.unpack_from('<I', d, q)[0]
    q += 4
    for _ in range(n_groups):
        mat = struct.unpack_from('<H', d, q)[0]
        nsub = struct.unpack_from('<I', d, q + 6)[0]
        q += 14
        subs = []
        for _ in range(nsub):
            c = struct.unpack_from('<I', d, q)[0]
            ns = struct.unpack_from('<I', d, q + 20)[0]
            q += 24
            subs.append((c, struct.unpack_from(f'<{ns}I', d, q)))
            q += 4 * ns
        groups.append((mat, subs))
    return (blobs, groups) if ((q + 3) & ~3) == end else None


def material_list(d, p):
    for _ in range(4):
        end, _ = chunk_at(d, p)
        kids = children(d, p, end)
        if not kids:
            return []
        q = kids[0][1] + 4
        if kids[0][0] == p + 12 and q + 8 <= end:
            n = struct.unpack_from('<I', d, q)[0]
            if 0 < n < 256 and struct.unpack_from('<H', d, q + 4)[0] == 0x1B:
                q += 4
                out = []
                for _ in range(n):
                    nl = struct.unpack_from('<I', d, q + 16)[0]
                    out.append(d[q + 20:q + 20 + nl].split(b'\0')[0].decode('latin1'))
                    q += 20 + nl + 4
                    inline = d[q]
                    q += 1
                    if inline:
                        q = chunk_at(d, (q + 3) & ~3)[0] + 4
                return out
        p = kids[0][0]
    return []


def level_objects(d):
    """Yield (name, [(material, verts, uvs, normals, tris)]) for each level object."""
    for i in range(0, len(d) - 12, 4):
        if d[i:i + 4] != BB:
            continue
        ch = chunk_at(d, i)
        if not ch or ch[1] != 3:
            continue
        kids = children(d, i, ch[0])
        if len(kids) < 3 or kids[1][2] != 4:
            continue
        rend = [k for k in children(d, kids[1][0], kids[1][1]) if k[2] == 1]
        if not rend:
            continue
        try:
            r = parse_render(d, rend[0][0])
        except (struct.error, IndexError):
            r = None
        if not r:
            continue
        (p0, p1, p2, p3), groups = r
        mats = material_list(d, kids[2][0])
        parts = {}
        o0 = o1 = o2 = o3 = 0
        for mat, subs in groups:
            mid = mat & 0x7FFF
            part = parts.setdefault(mid, (mats[mid] if mid < len(mats) else '', [], [], [], []))
            for c, lists in subs:
                base = len(part[1])
                for k in range(c):
                    x, y, z = struct.unpack_from('<3h', d, p0 + o0 + 6 * k)
                    part[1].append((x * UNIT, z * UNIT, -y * UNIT))
                    u, v = struct.unpack_from('<2h', d, p2 + o2 + 4 * k)
                    part[2].append((u / 4096, v / 4096))
                    nx, ny, nz = struct.unpack_from('<3h', d, p1 + o1 + 6 * k)
                    part[3].append((nx / 4096, nz / 4096, -ny / 4096))
                o0 += a16(6 * c)
                o1 += a16(6 * c)
                o2 += a16(4 * c)
                for n in lists:
                    st = d[p3 + o3:p3 + o3 + n]
                    o3 += a16(n)
                    for k in range(2, n):
                        if st[k] & 0x80:
                            continue        # strip restart
                        a, b, cc = st[k - 2] & 0x7F, st[k - 1] & 0x7F, st[k] & 0x7F
                        if max(a, b, cc) < c:
                            part[4].append((base + a, base + b, base + cc) if k % 2 == 0 else (base + b, base + a, base + cc))
        yield chunk_name(d, i, ch[0]) or f'obj_{i:x}', list(parts.values())


def material_textures(d):
    """material name -> texture name, from material (v5) chunks with an embedded texture (v3)."""
    out = {}
    for i in range(0, len(d) - 12, 4):
        if d[i:i + 4] != BB:
            continue
        c = chunk_at(d, i)
        if c and c[1] == 5:
            m = chunk_name(d, i, c[0])
            for kp, ke, kv in children(d, i, c[0]):
                t = chunk_name(d, kp, ke) if kv == 3 else None
                if m and t:
                    out.setdefault(m, t)
    return out


def main(argv):
    if len(argv) != 3:
        sys.exit(__doc__)
    d = open(argv[1], 'rb').read()
    out = argv[2]
    os.makedirs(os.path.join(out, 'tex'), exist_ok=True)
    # textures by name
    cont = Container(d)
    textures = {}
    for o, size, _ in cont.allocations():
        t = tex.parse(d, o, size)
        if t:
            textures.setdefault(cont.name_near(cont.refs[o][0]) or f'tex_{o:x}', t)
    m2t = material_textures(d)

    def find_tex(m):
        cands = [m2t.get(m), m] + [m.lower()[:-len(s)] for s in ('_ds', '_noalpha', '_shado', '_recproj') if m.lower().endswith(s)]
        for c in cands:
            if c:
                for k in textures:
                    if k.lower() == c.lower():
                        return k
        return None

    used = {}
    n_obj = n_tri = 0
    with open(os.path.join(out, 'level.obj'), 'w') as f:
        f.write('mtllib level.mtl\n')
        vb = 1
        for name, parts in level_objects(d):
            n_obj += 1
            f.write(f'o {name}\n')
            for mat, verts, uvs, nrms, tris in parts:
                t = find_tex(mat)
                key = t or 'untextured'
                used[key] = t
                f.write(f'usemtl {key}\n')
                for v in verts:
                    f.write('v %.4f %.4f %.4f\n' % v)
                for u, v in uvs:
                    f.write('vt %.4f %.4f\n' % (u, 1 - v))
                for n in nrms:
                    f.write('vn %.4f %.4f %.4f\n' % n)
                for a, b, c in tris:
                    f.write(f'f {a + vb}/{a + vb}/{a + vb} {b + vb}/{b + vb}/{b + vb} {c + vb}/{c + vb}/{c + vb}\n')
                vb += len(verts)
                n_tri += len(tris)
    with open(os.path.join(out, 'level.mtl'), 'w') as f:
        for key, t in used.items():
            f.write(f'newmtl {key}\nKd 1 1 1\n')
            if t:
                w, h, _, pix, pal = textures[t]
                tex.to_image(w, h, pix, pal).save(os.path.join(out, 'tex', f'{t}.png'))
                f.write(f'map_Kd tex/{t}.png\n')
    print(f'{n_obj} objects, {n_tri} triangles, {sum(1 for t in used.values() if t)} textures -> {out}/level.obj')


if __name__ == '__main__':
    main(sys.argv)
