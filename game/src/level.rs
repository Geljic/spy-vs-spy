//! Real levels: collision geometry decoded from the disc (notes/formats.md#collision),
//! plus the queries movement needs (ground height, wall push-out).

use bevy::math::{Vec2, Vec3};
use std::collections::HashMap;

/// int16 file units -> metres (calibrated: embassy rooms ~3.8 m floor to ceiling)
pub const UNIT: f32 = 0.006;
/// Engine units (entity positions, model space) -> metres. Level int16 coordinates are 1/128 of an
/// engine unit: every collision object's bounding radius is exactly max |vertex| / 128.
pub const ENGINE_UNIT: f32 = UNIT * 128.0;
const CELL: f32 = 2.0;

pub struct Tri {
    pub v: [Vec3; 3],
    pub n: Vec3,
    pub surface: u16,
}

pub struct Level {
    pub name: String,
    pub tris: Vec<Tri>,
    grid: HashMap<(i32, i32), Vec<u32>>,
    /// render geometry (has ceilings, which collision lacks): used for camera and "indoors" rays
    vis: Vec<[Vec3; 3]>,
    vis_grid: HashMap<(i32, i32), Vec<u32>>,
    pub min: Vec3,
    pub max: Vec3,
}

const BB: [u8; 4] = [0xBB; 4];
const BE: [u8; 4] = [0xBE; 4];

fn u32_at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}
fn i16x3(d: &[u8], o: usize) -> [i16; 3] {
    [0, 2, 4].map(|k| i16::from_le_bytes([d[o + k], d[o + k + 1]]))
}

fn chunk_at(d: &[u8], p: usize) -> Option<(usize, u32)> {
    if p % 4 != 0 || p + 12 > d.len() || d[p..p + 4] != BB {
        return None;
    }
    let end = u32_at(d, p + 4) as usize;
    (end > p && end + 4 <= d.len() && d[end..end + 4] == BE).then(|| (end, u32_at(d, p + 8)))
}

/// End of a model template chunk (v9: [v2 name] [object ref] [v6 model] [v1 materials]) starting at p.
pub(crate) fn template_end(d: &[u8], p: usize) -> Option<usize> {
    let (end, 9) = chunk_at(d, p)? else { return None };
    let (name_end, 2) = chunk_at(d, p + 12)? else { return None };
    // the v6 model follows the name and a short object reference
    let has_model = (name_end + 4..(name_end + 256).min(end)).step_by(4).any(|q| matches!(chunk_at(d, q), Some((_, 6))));
    has_model.then_some(end)
}

/// One collision object chunk -> triangles in file units (z-up). None if it doesn't parse exactly.
pub(crate) fn parse_collision(d: &[u8], p: usize, out: &mut Vec<([[i16; 3]; 3], u16)>) -> Option<()> {
    let (end, ver) = chunk_at(d, p)?;
    let (bend, bver) = chunk_at(d, p + 12)?;
    if bver != 1 {
        return None;
    }
    let mut q = bend + 4;
    if *d.get(q)? == 0 {
        return None;
    }
    q += 1;
    let n_nodes = u32_at(d, q + 8) as usize;
    q += 12 + n_nodes.checked_mul(28)?;
    let n_leaves = u32_at(d, *&q) as usize;
    q += 4;
    let mut leaves = Vec::with_capacity(n_leaves);
    for _ in 0..n_leaves {
        let n_polys = *d.get(q + 1)? as usize;
        let n_normals = (u32_at(d, q + 2) & 0xF) as usize + 1;
        leaves.push((n_polys, n_normals));
        q += 26;
    }
    q += 4;
    let desc_len = if ver > 3 { 10 } else { 8 };
    let start = out.len();
    for (n_polys, n_normals) in leaves {
        let descs: Vec<&[u8]> = (0..n_polys).map(|k| d.get(q + k * desc_len..q + (k + 1) * desc_len)).collect::<Option<_>>()?;
        q += n_polys * desc_len;
        let h = |s: &[u8], i: usize| u16::from_le_bytes([s[i * 2], s[i * 2 + 1]]);
        let last = desc_len / 2 - 1;
        let n_verts = descs.iter().map(|s| (h(s, last) & 0xF) as usize + 1).max().unwrap_or(0);
        q += 6 * n_normals;
        if q + 6 * n_verts > d.len() {
            return None;
        }
        let verts: Vec<[i16; 3]> = (0..n_verts).map(|k| i16x3(d, q + 6 * k)).collect();
        q += 6 * n_verts;
        for k in 2..descs.len() {
            if h(descs[k], 0) & 1 == 0 {
                continue;
            }
            let ids = [k - 2, k - 1, k].map(|j| (h(descs[j], last) & 0xF) as usize);
            let mut t = ids.map(|i| verts[i]);
            if h(descs[k], 1) & 1 != 0 {
                t.swap(1, 2);
            }
            out.push((t, h(descs[k], 2)));
        }
    }
    if (q + 3) & !3 != end {
        out.truncate(start);
        return None;
    }
    Some(())
}

impl Level {
    pub fn from_map(name: &str, d: &[u8]) -> Level {
        let mut raw = Vec::new();
        let mut i = 0;
        while i + 28 < d.len() {
            if d[i..i + 4] == BB {
                // model templates carry their own collision in model space: placed per entity, not here
                if let Some(end) = template_end(d, i) {
                    i = end + 4;
                    continue;
                }
                if let Some((_, 4)) = chunk_at(d, i) {
                    if let Some((kend, 1)) = chunk_at(d, i + 12) {
                        if kend - (i + 12) == 16 {
                            let _ = parse_collision(d, i, &mut raw);
                        }
                    }
                }
            }
            i += 4;
        }
        // file is z-up; Bevy is y-up (x, z, -y)
        let conv = |p: [i16; 3]| Vec3::new(p[0] as f32, p[2] as f32, -(p[1] as f32)) * UNIT;
        let mut tris = Vec::with_capacity(raw.len());
        for (t, surface) in raw {
            let v = t.map(conv);
            let n = (v[1] - v[0]).cross(v[2] - v[0]);
            if n.length_squared() < 1e-10 {
                continue;
            }
            tris.push(Tri { v, n: n.normalize(), surface });
        }
        let mut lvl = Level { name: name.into(), tris, grid: HashMap::new(), vis: Vec::new(), vis_grid: HashMap::new(), min: Vec3::splat(f32::MAX), max: Vec3::splat(f32::MIN) };
        for (k, t) in lvl.tris.iter().enumerate() {
            let lo = t.v[0].min(t.v[1]).min(t.v[2]);
            let hi = t.v[0].max(t.v[1]).max(t.v[2]);
            lvl.min = lvl.min.min(lo);
            lvl.max = lvl.max.max(hi);
            for cx in (lo.x / CELL).floor() as i32..=(hi.x / CELL).floor() as i32 {
                for cz in (lo.z / CELL).floor() as i32..=(hi.z / CELL).floor() as i32 {
                    lvl.grid.entry((cx, cz)).or_default().push(k as u32);
                }
            }
        }
        lvl
    }

    /// Add solid triangles (metres, Bevy axes) to the collision, e.g. placed objects' shapes.
    pub fn add_tris(&mut self, tris: impl IntoIterator<Item = [Vec3; 3]>) {
        for v in tris {
            let n = (v[1] - v[0]).cross(v[2] - v[0]);
            if n.length_squared() < 1e-8 {
                continue;
            }
            let k = self.tris.len() as u32;
            let lo = v[0].min(v[1]).min(v[2]);
            let hi = v[0].max(v[1]).max(v[2]);
            self.min = self.min.min(lo);
            self.max = self.max.max(hi);
            for cx in (lo.x / CELL).floor() as i32..=(hi.x / CELL).floor() as i32 {
                for cz in (lo.z / CELL).floor() as i32..=(hi.z / CELL).floor() as i32 {
                    self.grid.entry((cx, cz)).or_default().push(k);
                }
            }
            self.tris.push(Tri { v, n: n.normalize(), surface: 0 });
        }
    }

    pub fn add_visual(&mut self, tris: impl IntoIterator<Item = [Vec3; 3]>) {
        for v in tris {
            let k = self.vis.len() as u32;
            let lo = v[0].min(v[1]).min(v[2]);
            let hi = v[0].max(v[1]).max(v[2]);
            for cx in (lo.x / CELL).floor() as i32..=(hi.x / CELL).floor() as i32 {
                for cz in (lo.z / CELL).floor() as i32..=(hi.z / CELL).floor() as i32 {
                    self.vis_grid.entry((cx, cz)).or_default().push(k);
                }
            }
            self.vis.push(v);
        }
    }

    fn near_vis(&self, p: Vec2, r: f32) -> Vec<u32> {
        let mut out = Vec::new();
        for cx in ((p.x - r) / CELL).floor() as i32..=((p.x + r) / CELL).floor() as i32 {
            for cz in ((p.y - r) / CELL).floor() as i32..=((p.y + r) / CELL).floor() as i32 {
                if let Some(v) = self.vis_grid.get(&(cx, cz)) {
                    out.extend_from_slice(v);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    fn near(&self, p: Vec2, r: f32) -> Vec<u32> {
        let mut out = Vec::new();
        for cx in ((p.x - r) / CELL).floor() as i32..=((p.x + r) / CELL).floor() as i32 {
            for cz in ((p.y - r) / CELL).floor() as i32..=((p.y + r) / CELL).floor() as i32 {
                if let Some(v) = self.grid.get(&(cx, cz)) {
                    out.extend_from_slice(v);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Highest walkable surface under (x, z) that is no higher than `below`.
    pub fn ground(&self, p: Vec3, below: f32) -> Option<f32> {
        let mut best: Option<f32> = None;
        for k in self.near(Vec2::new(p.x, p.z), 0.0) {
            let t = &self.tris[k as usize];
            if t.n.y < 0.45 {
                continue;
            }
            if let Some(y) = height_on(t, p.x, p.z) {
                if y <= below && best.is_none_or(|b| y > b) {
                    best = Some(y);
                }
            }
        }
        best
    }

    /// Push a vertical capsule (feet at p) out of walls. Returns the corrected position.
    pub fn push_out(&self, mut p: Vec3, radius: f32, height: f32) -> Vec3 {
        for _ in 0..3 {
            let mut moved = false;
            for k in self.near(Vec2::new(p.x, p.z), radius + 0.5) {
                let t = &self.tris[k as usize];
                if t.n.y > 0.45 {
                    continue; // floors are handled by ground()
                }
                for h in [0.35, height * 0.5, height - 0.2] {
                    let c = p + Vec3::Y * h;
                    let q = closest_on_tri(c, t);
                    let d = c - q;
                    let mut d2 = Vec3::new(d.x, 0.0, d.z);
                    let dist = d2.length();
                    if dist < radius && (q.y - c.y).abs() < 0.3 {
                        if dist < 1e-4 {
                            d2 = Vec3::new(t.n.x, 0.0, t.n.z);
                        }
                        p += d2.normalize_or_zero() * (radius - dist);
                        moved = true;
                    }
                }
            }
            if !moved {
                break;
            }
        }
        p
    }

    /// Distance along `dir` (unit) to the first triangle hit within `max`, if any.
    pub fn raycast(&self, from: Vec3, dir: Vec3, max: f32) -> Option<f32> {
        let mut cand: Vec<u32> = Vec::new();
        let mut vcand: Vec<u32> = Vec::new();
        let steps = (max / (CELL * 0.5)).ceil() as i32;
        for k in 0..=steps {
            let p = from + dir * (k as f32 * CELL * 0.5).min(max);
            cand.extend(self.near(Vec2::new(p.x, p.z), CELL * 0.5));
            vcand.extend(self.near_vis(Vec2::new(p.x, p.z), CELL * 0.5));
        }
        cand.sort_unstable();
        cand.dedup();
        vcand.sort_unstable();
        vcand.dedup();
        let tris = cand.iter().map(|&k| self.tris[k as usize].v).chain(vcand.iter().map(|&k| self.vis[k as usize]));
        let mut best: Option<f32> = None;
        for [a, b, c] in tris {
            // Moller-Trumbore
            let (e1, e2) = (b - a, c - a);
            let pv = dir.cross(e2);
            let det = e1.dot(pv);
            if det.abs() < 1e-8 {
                continue;
            }
            let inv = 1.0 / det;
            let tv = from - a;
            let u = tv.dot(pv) * inv;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let qv = tv.cross(e1);
            let v = dir.dot(qv) * inv;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let t = e2.dot(qv) * inv;
            if t > 0.0 && t < max && best.is_none_or(|b| t < b) {
                best = Some(t);
            }
        }
        best
    }

    /// Nothing solid between `y + 0.1` and `y + h` above (x, z)?
    fn headroom(&self, p: Vec3, h: f32) -> bool {
        self.near(Vec2::new(p.x, p.z), 0.0).iter().all(|&k| {
            height_on(&self.tris[k as usize], p.x, p.z).is_none_or(|y| y < p.y + 0.1 || y > p.y + h)
        })
    }

    /// Nearest walkable floor point to `p` (within `r` metres), for snapping entity start points.
    pub fn nearest_floor(&self, p: Vec3, r: f32) -> Option<Vec3> {
        let mut best: Option<(f32, Vec3)> = None;
        for k in self.near(Vec2::new(p.x, p.z), r) {
            let t = &self.tris[k as usize];
            if t.n.y < 0.7 {
                continue;
            }
            let q = closest_on_tri(p, t);
            let d = q.distance(p);
            if d < r && best.is_none_or(|(bd, _)| d < bd) && self.headroom(q, 1.9) {
                best = Some((d, q));
            }
        }
        best.map(|(_, q)| q)
    }

    /// A walkable, open spot near the middle of the level (for spawning).
    pub fn spawn_point(&self, k: usize) -> Vec3 {
        let c = (self.min + self.max) * 0.5;
        let mut spots: Vec<Vec3> = self
            .tris
            .iter()
            .filter(|t| t.n.y > 0.9 && (t.v[1] - t.v[0]).cross(t.v[2] - t.v[0]).length() > 2.0)
            .map(|t| (t.v[0] + t.v[1] + t.v[2]) / 3.0)
            // standing room, and not wedged against walls
            .filter(|&p| self.headroom(p, 2.6) && self.push_out(p, 1.2, 1.9).distance(p) < 1e-3)
            .collect();
        // prefer indoors (a ceiling overhead) when the level has any: avoids starting on rooftops
        let indoor: Vec<Vec3> = spots.iter().copied().filter(|&p| self.raycast(p + Vec3::Y * 0.2, Vec3::Y, 12.0).is_some()).collect();
        if indoor.len() >= 10 {
            spots = indoor;
        }
        spots.sort_by(|a, b| Vec2::new(a.x - c.x, a.z - c.z).length().total_cmp(&Vec2::new(b.x - c.x, b.z - c.z).length()));
        let p = spots.get((k * 5).min(spots.len().saturating_sub(1))).copied().unwrap_or(c);
        Vec3::new(p.x, p.y + 0.05, p.z)
    }
}

fn height_on(t: &Tri, x: f32, z: f32) -> Option<f32> {
    let [a, b, c] = t.v;
    let (v0, v1) = (Vec2::new(b.x - a.x, b.z - a.z), Vec2::new(c.x - a.x, c.z - a.z));
    let v2 = Vec2::new(x - a.x, z - a.z);
    let den = v0.x * v1.y - v1.x * v0.y;
    if den.abs() < 1e-9 {
        return None;
    }
    let u = (v2.x * v1.y - v1.x * v2.y) / den;
    let v = (v0.x * v2.y - v2.x * v0.y) / den;
    (u >= -1e-4 && v >= -1e-4 && u + v <= 1.0001).then(|| a.y + u * (b.y - a.y) + v * (c.y - a.y))
}

fn closest_on_tri(p: Vec3, t: &Tri) -> Vec3 {
    // Ericson, Real-Time Collision Detection 5.1.5
    let [a, b, c] = t.v;
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let den = 1.0 / (va + vb + vc);
    a + ab * (vb * den) + ac * (vc * den)
}
