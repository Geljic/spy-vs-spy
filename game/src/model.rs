//! Level render meshes + materials decoded from a .map (notes/formats.md#render-mesh).
//!
//! Level object (v3 chunk): [v1 name] [v4 geometry] [v1 material list] [v1 ...]
//!   geometry (v4): [v3 base] u32 n_lods + lods, [v1 render data], u8 has_collision + collision
//!   render data (v1): u8 present, 4 x (u32 size + blob), u32 n_groups, groups
//!     blob0 positions i16x3, blob1 normals i16x3 (/4096), blob2 uv i16x2 (/4096), blob3 strip indices u8
//!     every batch's slice of each blob (and every index list) is padded to 16 bytes
//!   material list: name chunk, u32 count, count x (object ref + u8 inline-definition flag [+ v5 chunk])

use crate::level::{ENGINE_UNIT, UNIT};
use std::collections::HashMap;

const BB: [u8; 4] = [0xBB; 4];
const BE: [u8; 4] = [0xBE; 4];

fn u16_at(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}
fn u32_at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}
fn i16_at(d: &[u8], o: usize) -> f32 {
    i16::from_le_bytes([d[o], d[o + 1]]) as f32
}
fn a16(n: usize) -> usize {
    (n + 15) & !15
}

fn chunk_at(d: &[u8], p: usize) -> Option<(usize, u32)> {
    if p % 4 != 0 || p + 12 > d.len() || d[p..p + 4] != BB {
        return None;
    }
    let end = u32_at(d, p + 4) as usize;
    (end > p && end + 4 <= d.len() && d[end..end + 4] == BE).then(|| (end, u32_at(d, p + 8)))
}

/// Direct child chunks of the chunk at p: (start, end, version).
fn children(d: &[u8], p: usize, end: usize) -> Vec<(usize, usize, u32)> {
    let mut out = Vec::new();
    let mut q = p + 12;
    while q < end {
        q = (q + 3) & !3;
        if let Some((e, v)) = chunk_at(d, q) {
            out.push((q, e, v));
            q = e + 4;
        } else {
            q += 4;
        }
    }
    out
}

/// `u32 0x14` + 20-byte name near the start of a chunk.
fn chunk_name(d: &[u8], p: usize, end: usize) -> Option<String> {
    let hay = &d[p..end.min(p + 120)];
    let j = hay.windows(4).position(|w| w == [0x14, 0, 0, 0])? + p;
    let raw = &d[j + 4..j + 24];
    let raw = &raw[..raw.iter().position(|&b| b == 0).unwrap_or(20)];
    (!raw.is_empty() && raw.iter().all(|&c| c > 32 && c < 127)).then(|| String::from_utf8_lossy(raw).into_owned())
}

/// Triangles of one material inside one object.
#[derive(Default)]
pub struct Part {
    pub object: String,
    pub material: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

pub struct LevelModels {
    pub parts: Vec<Part>,
    /// material name -> texture name
    pub mat_tex: HashMap<String, String>,
    pub objects: usize,
}

struct Group {
    mat: u16,
    subs: Vec<(usize, Vec<usize>)>, // (vertex count, index list lengths)
}

fn parse_render(d: &[u8], p: usize) -> Option<([(usize, usize); 4], Vec<Group>)> {
    let (end, _) = chunk_at(d, p)?;
    let mut q = p + 12;
    if *d.get(q)? == 0 {
        return None;
    }
    q += 1;
    let mut blobs = [(0, 0); 4];
    for b in &mut blobs {
        let n = u32_at(d, q) as usize;
        *b = (q + 4, n);
        q += 4 + n;
        if q > end {
            return None;
        }
    }
    let ng = u32_at(d, q) as usize;
    q += 4;
    let mut groups = Vec::with_capacity(ng);
    for _ in 0..ng {
        let mat = u16_at(d, q);
        let nsub = u32_at(d, q + 6) as usize;
        q += 14;
        let mut subs = Vec::with_capacity(nsub);
        for _ in 0..nsub {
            let c = u32_at(d, q) as usize;
            let ns = u32_at(d, q + 20) as usize;
            q += 24;
            if q + 4 * ns > end || c > 128 {
                return None;
            }
            subs.push((c, (0..ns).map(|k| u32_at(d, q + 4 * k) as usize).collect()));
            q += 4 * ns;
        }
        groups.push(Group { mat, subs });
    }
    ((q + 3) & !3 == end).then_some((blobs, groups))
}

/// Material names of a level object, in the order its groups index them.
fn material_list(d: &[u8], p: usize) -> Option<Vec<String>> {
    // the list sits a couple of wrapper chunks down; it starts with a name chunk then a count
    let mut p = p;
    for _ in 0..4 {
        let (end, _) = chunk_at(d, p)?;
        let kids = children(d, p, end);
        let first = *kids.first()?;
        let q = first.1 + 4;
        if first.0 == p + 12 && q + 4 <= end {
            let n = u32_at(d, q) as usize;
            if n > 0 && n < 256 && q + 8 <= end && u16_at(d, q + 4) == 0x1b {
                let mut q = q + 4;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    let nl = u32_at(d, q + 16) as usize;
                    let raw = &d[q + 20..q + 20 + nl];
                    out.push(String::from_utf8_lossy(&raw[..raw.iter().position(|&b| b == 0).unwrap_or(nl)]).into_owned());
                    q += 20 + nl + 4;
                    let inline = d[q];
                    q += 1;
                    if inline != 0 {
                        q = (q + 3) & !3;
                        q = chunk_at(d, q)?.0 + 4;
                    }
                }
                return Some(out);
            }
        }
        p = first.0;
    }
    None
}

pub fn load(d: &[u8]) -> LevelModels {
    let mut parts: Vec<Part> = Vec::new();
    let mut mat_tex = HashMap::new();
    let mut objects = 0;
    let mut i = 0;
    while i + 12 < d.len() {
        if d[i..i + 4] != BB {
            i += 4;
            continue;
        }
        let Some((end, ver)) = chunk_at(d, i) else {
            i += 4;
            continue;
        };
        // materials: v5 chunk with a v3 texture child
        if ver == 5 {
            if let Some(m) = chunk_name(d, i, end) {
                for (kp, ke, kv) in children(d, i, end) {
                    if kv == 3 {
                        if let Some(t) = chunk_name(d, kp, ke) {
                            mat_tex.entry(m.clone()).or_insert(t);
                        }
                    }
                }
            }
        }
        if ver == 3 {
            if let Some(obj) = load_object(d, i, end) {
                objects += 1;
                parts.extend(obj);
            }
        }
        i += 4;
    }
    LevelModels { parts, mat_tex, objects }
}

fn load_object(d: &[u8], p: usize, end: usize) -> Option<Vec<Part>> {
    let kids = children(d, p, end);
    if kids.len() < 3 || kids[1].2 != 4 {
        return None;
    }
    let geo = kids[1];
    let render = children(d, geo.0, geo.1).into_iter().find(|k| k.2 == 1 && k.0 != geo.0 + 12)?;
    let (blobs, groups) = parse_render(d, render.0)?;
    let mats = material_list(d, kids[2].0).unwrap_or_default();
    let object = chunk_name(d, p, end).unwrap_or_default();

    let mut by_mat: HashMap<u16, Part> = HashMap::new();
    let (mut o0, mut o1, mut o2, mut o3) = (0, 0, 0, 0);
    for g in &groups {
        let id = g.mat & 0x7fff;
        let part = by_mat.entry(id).or_insert_with(|| Part { object: object.clone(), material: mats.get(id as usize).cloned().unwrap_or_default(), ..Default::default() });
        for (c, lists) in &g.subs {
            let c = *c;
            let base = part.positions.len() as u32;
            for k in 0..c {
                let pp = blobs[0].0 + o0 + 6 * k;
                let np = blobs[1].0 + o1 + 6 * k;
                let up = blobs[2].0 + o2 + 4 * k;
                if pp + 6 > d.len() || np + 6 > d.len() || up + 4 > d.len() {
                    return None;
                }
                // file is z-up; Bevy is y-up
                part.positions.push([i16_at(d, pp) * UNIT, i16_at(d, pp + 4) * UNIT, -i16_at(d, pp + 2) * UNIT]);
                part.normals.push([i16_at(d, np) / 4096.0, i16_at(d, np + 4) / 4096.0, -i16_at(d, np + 2) / 4096.0]);
                part.uvs.push([i16_at(d, up) / 4096.0, i16_at(d, up + 2) / 4096.0]);
            }
            o0 += a16(6 * c);
            o1 += a16(6 * c);
            o2 += a16(4 * c);
            for &len in lists {
                let s = blobs[3].0 + o3;
                o3 += a16(len);
                if s + len > d.len() {
                    return None;
                }
                let st = &d[s..s + len];
                for k in 2..len {
                    if st[k] & 0x80 != 0 {
                        continue; // strip restart: no triangle here
                    }
                    let (a, b, cc) = ((st[k - 2] & 0x7f) as usize, (st[k - 1] & 0x7f) as usize, (st[k] & 0x7f) as usize);
                    if a.max(b).max(cc) >= c {
                        continue;
                    }
                    let (a, b) = if k % 2 == 0 { (a, b) } else { (b, a) };
                    part.indices.extend([base + a as u32, base + b as u32, base + cc as u32]);
                }
            }
        }
    }
    Some(by_mat.into_values().filter(|p| !p.indices.is_empty()).collect())
}

impl Part {
    /// Sky domes: drawn unlit, ignored by camera and "indoors" rays.
    pub fn is_sky(&self) -> bool {
        self.object.starts_with("h_skybox")
    }
}

fn f32_at(d: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}

/// Rigid transform in file space (z-up, engine units): origin + the X, Y, Z axes.
#[derive(Clone, Copy)]
struct Xf {
    p: [f32; 3],
    r: [[f32; 3]; 3],
}

impl Xf {
    /// float3 origin then three float3 axes, as stored in model nodes and shapes
    fn at(d: &[u8], q: usize) -> Xf {
        let v = |k: usize| [0, 1, 2].map(|i| f32_at(d, q + 12 * k + 4 * i));
        Xf { p: v(0), r: [v(1), v(2), v(3)] }
    }
    fn rot(&self, v: [f32; 3]) -> [f32; 3] {
        [0, 1, 2].map(|i| v[0] * self.r[0][i] + v[1] * self.r[1][i] + v[2] * self.r[2][i])
    }
    fn apply(&self, v: [f32; 3]) -> [f32; 3] {
        let r = self.rot(v);
        [0, 1, 2].map(|i| self.p[i] + r[i])
    }
    /// this transform applied after `local` (local is expressed in this one's space)
    fn then(&self, local: &Xf) -> Xf {
        Xf { p: self.apply(local.p), r: local.r.map(|a| self.rot(a)) }
    }
}

/// A model template (v9 chunk: [v2 name] [v6 model] [v1 materials]) ready to place.
/// Everything is in template space: file axes (z-up), engine units. See notes/formats.md#model.
pub struct Template {
    /// rigid render geometry, one part per material slot (skinned geometry: not yet)
    pub parts: Vec<Part>,
    /// solid shapes (boxes, spheres/capsules as boxes, embedded collision meshes) as triangles
    pub collision: Vec<[[f32; 3]; 3]>,
}

/// Every model template in a map, by template name.
pub fn templates(d: &[u8]) -> HashMap<String, Template> {
    let mut out = HashMap::new();
    let mut i = 0;
    while i + 12 < d.len() {
        if d[i..i + 4] == BB {
            if let Some(end) = crate::level::template_end(d, i) {
                let kids = children(d, i, end);
                let mi = kids.iter().position(|k| k.2 == 6);
                if let (Some(name), Some(mi)) = (chunk_name(d, kids[0].0, kids[0].1), mi) {
                    let model = &kids[mi];
                    let mats = kids.get(mi + 1).and_then(|m| material_list(d, m.0)).unwrap_or_default();
                    if let Some(t) = parse_model(d, model.0, model.1, model.2, &name, &mats) {
                        out.entry(name).or_insert(t);
                    }
                }
                i = end + 4;
                continue;
            }
        }
        i += 4;
    }
    out
}

/// v6 model chunk (loader FUN_00209660 "LoadModelChunkV6").
fn parse_model(d: &[u8], p: usize, end: usize, ver: u32, name: &str, mats: &[String]) -> Option<Template> {
    let kids = children(d, p, end);
    let base = *kids.first()?;
    let mut q = base.1 + 4;
    // vertex quantisation: int16 steps of 2^ceil(log2(scale)) / 32768
    let step = f32_at(d, q).log2().ceil().exp2() / 32768.0;
    q += 12 + 10;
    // nodes: u32 parent (-1 = root), local transform, inline name
    let n = u32_at(d, q) as usize;
    q += 4;
    if q + n * 80 > end {
        return None;
    }
    let mut world: Vec<Xf> = Vec::with_capacity(n);
    for k in 0..n {
        let parent = u32_at(d, q + 80 * k) as usize;
        let local = Xf::at(d, q + 80 * k + 4);
        let w = if parent < k { world[parent].then(&local) } else { local };
        world.push(w);
    }
    q += 80 * n;
    // bones (inverse bind matrices) and attach points: not used yet
    let n = u32_at(d, q) as usize;
    q += 4 + 48 * n;
    let n = u32_at(d, q) as usize;
    q += 4 + n * if ver >= 4 { 116 } else { 80 };
    // collision shapes: u32 node, transform (offset + axes), float3 params, u16 type, u16
    let mut collision = Vec::new();
    if ver > 1 {
        let n = u32_at(d, q) as usize;
        q += 4;
        for _ in 0..n {
            if q + 68 > end {
                return None;
            }
            let node = world.get(u32_at(d, q) as usize).copied().unwrap_or(Xf { p: [0.0; 3], r: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]] });
            let xf = node.then(&Xf::at(d, q + 4));
            let prm = [0, 1, 2].map(|i| f32_at(d, q + 52 + 4 * i));
            let typ = u16_at(d, q + 64);
            q += 68;
            let half = match typ {
                0 => Some([prm[0]; 3]),              // sphere: radius
                1 => Some([prm[0], prm[0], prm[2]]), // capsule along z: radius, half length incl. caps
                2 => Some(prm),                      // box: half extents
                _ => None,
            };
            if let Some(h) = half {
                collision.extend(box_tris(h).into_iter().map(|t| t.map(|v| xf.apply(v))));
            }
            if ver > 5 && typ == 3 {
                // embedded collision mesh (same chunk as level collision), int16 = 1/128 unit
                q = (q + 3) & !3;
                let (cend, _) = chunk_at(d, q)?;
                let mut raw = Vec::new();
                crate::level::parse_collision(d, q, &mut raw)?;
                collision.extend(raw.into_iter().map(|(t, _)| t.map(|v| xf.apply(v.map(|c| c as f32 / 128.0)))));
                q = cend + 4;
            }
        }
    }
    // geometry: the v6 chunk's last v1 child; rigid part first
    let g = *kids.iter().rev().find(|k| k.2 == 1)?;
    let mut q = g.0 + 12;
    let mut parts: HashMap<usize, Part> = HashMap::new();
    if *d.get(q)? != 0 && u32_at(d, q + 1) > 0 {
        let counts = d.get(q + 5..q + 37)?.to_vec();
        q += 37;
        let mut blobs = [(0usize, 0usize); 4];
        for b in &mut blobs {
            let n = u32_at(d, q) as usize;
            *b = (q + 4, n);
            q += 4 + n;
        }
        if q > g.1 {
            return None;
        }
        let (mut o0, mut o1, mut o2, mut o3) = (0, 0, 0, 0);
        for (slot, &count) in counts.iter().enumerate() {
            let part = parts.entry(slot).or_insert_with(|| Part { object: name.into(), material: mats.get(slot).cloned().unwrap_or_default(), ..Default::default() });
            for _ in 0..count {
                // batch: u16 node, u32, u32 n_sub, u32, then sub-batches as in level render data
                let node = world.get(u16_at(d, q) as usize).copied().unwrap_or(Xf { p: [0.0; 3], r: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]] });
                let nsub = u32_at(d, q + 6) as usize;
                q += 14;
                for _ in 0..nsub {
                    let c = u32_at(d, q) as usize;
                    let nl = u32_at(d, q + 20) as usize;
                    q += 24;
                    if c > 128 || q + 4 * nl > g.1 {
                        return None;
                    }
                    let lists: Vec<usize> = (0..nl).map(|k| u32_at(d, q + 4 * k) as usize).collect();
                    q += 4 * nl;
                    let base = part.positions.len() as u32;
                    for k in 0..c {
                        let (pp, np, up) = (blobs[0].0 + o0 + 6 * k, blobs[1].0 + o1 + 6 * k, blobs[2].0 + o2 + 4 * k);
                        if pp + 6 > d.len() || np + 6 > d.len() || up + 4 > d.len() {
                            return None;
                        }
                        part.positions.push(node.apply([0, 2, 4].map(|o| i16_at(d, pp + o) * step)));
                        part.normals.push(node.rot([0, 2, 4].map(|o| i16_at(d, np + o) / 4096.0)));
                        part.uvs.push([i16_at(d, up) / 4096.0, i16_at(d, up + 2) / 4096.0]);
                    }
                    o0 += a16(6 * c);
                    o1 += a16(6 * c);
                    o2 += a16(4 * c);
                    for len in lists {
                        let s = blobs[3].0 + o3;
                        o3 += a16(len);
                        let st = d.get(s..s + len)?;
                        for k in 2..len {
                            if st[k] & 0x80 != 0 {
                                continue; // strip restart
                            }
                            let (a, b, cc) = ((st[k - 2] & 0x7f) as u32, (st[k - 1] & 0x7f) as u32, (st[k] & 0x7f) as u32);
                            if a.max(b).max(cc) as usize >= c {
                                continue;
                            }
                            let (a, b) = if k % 2 == 0 { (a, b) } else { (b, a) };
                            part.indices.extend([base + a, base + b, base + cc]);
                        }
                    }
                }
            }
        }
    }
    let parts = parts.into_values().filter(|p| !p.indices.is_empty()).collect();
    Some(Template { parts, collision })
}

/// The 12 triangles of a box with these half extents, centred on the origin (outward winding).
fn box_tris(h: [f32; 3]) -> Vec<[[f32; 3]; 3]> {
    let c = |i: usize| [if i & 1 != 0 { h[0] } else { -h[0] }, if i & 2 != 0 { h[1] } else { -h[1] }, if i & 4 != 0 { h[2] } else { -h[2] }];
    let quads = [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]];
    quads.iter().flat_map(|q| [[c(q[0]), c(q[1]), c(q[2])], [c(q[0]), c(q[2]), c(q[3])]]).collect()
}

/// A placed gameplay entity (v13 chunk, loader FUN_0015a848): name, class it instantiates, transform.
pub struct Entity {
    pub name: String,
    pub class: String,
    /// model template of the class (from the class definition, inline on its first entity)
    pub template: Option<String>,
    /// position in metres, Bevy axes (y up)
    pub pos: [f32; 3],
    /// the entity's X, Y, Z axes in Bevy space
    pub axes: [[f32; 3]; 3],
}

/// Object reference (FUN_0013e0a8): i16 type, u16 flags, [u64 id, (u32), string, u8 inline (+ chunk)].
/// Returns (name, position after the ref, start of the inline definition chunk if present).
fn read_ref(d: &[u8], q: usize) -> Option<(Option<String>, usize, Option<usize>)> {
    let typ = i16::from_le_bytes([*d.get(q)?, *d.get(q + 1)?]);
    let fl = u16_at(d, q + 2);
    let mut q = q + 4;
    if typ < 0 {
        return Some((None, q, None));
    }
    q += 8;
    if fl != 0 && fl & 0x8000 == 0 {
        q += 4;
    }
    let nl = u32_at(d, q) as usize;
    let raw = d.get(q + 4..q + 4 + nl)?;
    let name = String::from_utf8_lossy(&raw[..raw.iter().position(|&b| b == 0).unwrap_or(nl)]).into_owned();
    q += 4 + nl + 4;
    let inline = *d.get(q)?;
    q += 1;
    let mut def = None;
    if inline != 0 {
        q = (q + 3) & !3;
        def = Some(q);
        q = chunk_at(d, q)?.0 + 4;
    }
    Some((Some(name), q, def))
}

pub fn entities(d: &[u8]) -> Vec<Entity> {
    let mut out = Vec::new();
    // class -> template, read from class definitions (inline on the first entity of each class)
    let mut class_tpl: HashMap<String, String> = HashMap::new();
    let mut i = 0;
    while i + 12 < d.len() {
        if d[i..i + 4] == BB {
            if let Some((end, 13)) = chunk_at(d, i) {
                if let Some((e, def)) = parse_entity(d, i, end) {
                    if let Some(t) = def.and_then(|p| class_template(d, p)) {
                        class_tpl.entry(e.class.clone()).or_insert(t);
                    }
                    out.push(e);
                }
            }
        }
        i += 4;
    }
    for e in &mut out {
        e.template = class_tpl.get(&e.class).cloned();
    }
    out
}

/// Class definition chunk (v16): [name chunk] u8, then an object ref to the model template.
fn class_template(d: &[u8], p: usize) -> Option<String> {
    let (end, _) = chunk_at(d, p)?;
    let first = *children(d, p, end).first()?;
    read_ref(d, first.1 + 4 + 1)?.0
}

/// The entity and, when its class definition is stored inline, where that chunk starts.
fn parse_entity(d: &[u8], p: usize, end: usize) -> Option<(Entity, Option<usize>)> {
    let name = chunk_name(d, p, end)?;
    let base = *children(d, p, end).first()?;
    let (class, q, def) = read_ref(d, base.1 + 4)?;
    let (_, q, _) = read_ref(d, q + 1)?;
    let q = q + 3 + 4 + 4 + 4;
    if q + 48 > end {
        return None;
    }
    let f = |k: usize| f32::from_le_bytes(d[q + 4 * k..q + 4 * k + 4].try_into().unwrap());
    // file is z-up in engine units; Bevy is y-up metres
    let s = ENGINE_UNIT;
    let conv = |x: f32, y: f32, z: f32| [x, z, -y];
    let e = Entity {
        name,
        class: class.unwrap_or_default(),
        template: None,
        pos: conv(f(0) * s, f(1) * s, f(2) * s),
        axes: [conv(f(3), f(4), f(5)), conv(f(6), f(7), f(8)), conv(f(9), f(10), f(11))],
    };
    Some((e, def))
}

impl Entity {
    /// A point in this entity's template space (file axes, engine units) -> world metres, Bevy axes.
    pub fn place(&self, v: [f32; 3]) -> [f32; 3] {
        [0, 1, 2].map(|i| self.pos[i] + ENGINE_UNIT * (v[0] * self.axes[0][i] + v[1] * self.axes[1][i] + v[2] * self.axes[2][i]))
    }
    /// A direction in template space -> Bevy axes.
    pub fn turn(&self, v: [f32; 3]) -> [f32; 3] {
        [0, 1, 2].map(|i| v[0] * self.axes[0][i] + v[1] * self.axes[1][i] + v[2] * self.axes[2][i])
    }
}
