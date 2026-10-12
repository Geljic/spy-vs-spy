//! Reads assets straight from the player's own disc image (ISO/MDF) at runtime.
//! Format notes: notes/formats.md. Nothing from the game is bundled with this program.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const SECTOR: u64 = 0x800;
const PTR: [u8; 4] = [0xBB; 4];
const GUARD: [u8; 4] = [0xBE; 4];

/// Disc search order: $SVS_DISC, next to the executable, then common folders.
pub fn find_disc() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SVS_DISC") {
        return Some(PathBuf::from(p));
    }
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            dirs.push(d.to_path_buf());
        }
    }
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let home = PathBuf::from(home);
        dirs.push(home.join("Games/Downloads/Spy vs. Spy"));
        dirs.push(home.join("Games/Spy vs. Spy"));
    }
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut found: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                matches!(
                    p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
                    Some("iso" | "mdf")
                )
            })
            .collect();
        // prefer an untouched .mdf dump over a patched .iso
        found.sort_by_key(|p| (p.extension().is_some_and(|e| !e.eq_ignore_ascii_case("mdf")), p.clone()));
        if let Some(p) = found.into_iter().next() {
            return Some(p);
        }
    }
    None
}

pub struct Pak {
    file: File,
    base: u64,
    entries: HashMap<String, (u64, u64)>,
}

impl Pak {
    pub fn open(disc: &Path) -> std::io::Result<Pak> {
        let mut file = File::open(disc)?;
        let base = find_iso_file(&mut file, b"DATA.PAK;1")?;
        file.seek(SeekFrom::Start(base))?;
        let mut hdr = [0u8; 8];
        file.read_exact(&mut hdr)?;
        let count = u32::from_le_bytes(hdr[0..4].try_into().unwrap()) as usize;
        let mut toc = vec![0u8; count * 48];
        file.seek(SeekFrom::Start(base + SECTOR))?;
        file.read_exact(&mut toc)?;
        let mut entries = HashMap::new();
        for e in toc.chunks_exact(48) {
            let end = e[..40].iter().position(|&b| b == 0).unwrap_or(40);
            let name = String::from_utf8_lossy(&e[..end]).into_owned();
            let sector = u32::from_le_bytes(e[40..44].try_into().unwrap()) as u64;
            let size = u32::from_le_bytes(e[44..48].try_into().unwrap()) as u64;
            entries.insert(name, (sector, size));
        }
        Ok(Pak { file, base, entries })
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    pub fn read(&mut self, name: &str) -> std::io::Result<Vec<u8>> {
        let (sector, size) = *self
            .entries
            .get(name)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, name.to_string()))?;
        self.file.seek(SeekFrom::Start(self.base + sector * SECTOR))?;
        let mut buf = vec![0u8; size as usize];
        self.file.read_exact(&mut buf)?;
        Ok(buf)
    }
}

fn find_iso_file(f: &mut File, name: &[u8]) -> std::io::Result<u64> {
    let bad = |m: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, m.to_string());
    let mut pvd = vec![0u8; SECTOR as usize];
    f.seek(SeekFrom::Start(16 * SECTOR))?;
    f.read_exact(&mut pvd)?;
    if &pvd[1..6] != b"CD001" {
        return Err(bad("not an ISO9660 image"));
    }
    let root = &pvd[156..190];
    let lba = u32::from_le_bytes(root[2..6].try_into().unwrap()) as u64;
    let size = u32::from_le_bytes(root[10..14].try_into().unwrap()) as usize;
    let mut dir = vec![0u8; size];
    f.seek(SeekFrom::Start(lba * SECTOR))?;
    f.read_exact(&mut dir)?;
    let mut i = 0;
    while i < dir.len() {
        let len = dir[i] as usize;
        if len == 0 {
            i = (i / SECTOR as usize + 1) * SECTOR as usize;
            continue;
        }
        let nlen = dir[i + 32] as usize;
        if &dir[i + 33..i + 33 + nlen] == name {
            return Ok(u32::from_le_bytes(dir[i + 2..i + 6].try_into().unwrap()) as u64 * SECTOR);
        }
        i += len;
    }
    Err(bad("DATA.PAK not found on disc"))
}

/// A decoded RGBA8 texture.
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Textures and sounds from one container file, keyed by their in-game names.
#[derive(Default)]
pub struct Assets {
    pub textures: HashMap<String, Texture>,
    pub sounds: HashMap<String, Vec<u8>>, // complete WAV files
}

fn u32_at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}

/// Scan a container (.map etc.) for textures and sound effects.
pub fn load_container(d: &[u8]) -> Assets {
    // pointer targets partition the file into allocations
    let mut refs: HashMap<usize, usize> = HashMap::new(); // target -> first pointer offset
    let mut i = 0;
    while i + 12 <= d.len() {
        if d[i..i + 4] == PTR {
            let off = u32_at(d, i + 4) as usize;
            if off + 4 <= d.len() && d[off..off + 4] == GUARD {
                refs.entry(off).or_insert(i);
            }
        }
        i += 4;
    }
    let mut targets: Vec<usize> = refs.keys().copied().collect();
    targets.sort_unstable();

    // texture objects are v3 chunks; name each texture by the innermost one containing its pixels
    // (the nearest preceding name is sometimes a neighbour's, which mislabelled e.g. the mansion floors)
    let mut tex_chunks: Vec<(usize, usize, String)> = Vec::new();
    let mut k = 0;
    while k + 12 <= d.len() {
        if d[k..k + 4] == PTR && k % 4 == 0 {
            let end = u32_at(d, k + 4) as usize;
            if u32_at(d, k + 8) == 3 && end > k && end + 4 <= d.len() && d[end..end + 4] == GUARD {
                if let Some(n) = name_after(d, k, end) {
                    tex_chunks.push((k, end, n));
                }
            }
        }
        k += 4;
    }
    let owner = |o: usize| -> Option<String> {
        tex_chunks.iter().filter(|(s, e, _)| *s < o && o < *e).min_by_key(|(s, e, _)| e - s).map(|(_, _, n)| n.clone())
    };

    let mut out = Assets::default();
    for w in targets.windows(2) {
        let (o, size) = (w[0], w[1] - w[0]);
        let name = || name_near(d, refs[&o]);
        if let Some(t) = parse_texture(d, o, size) {
            if let Some(n) = owner(o).or_else(name) {
                out.textures.entry(n).or_insert(t);
            }
        } else if let Some(wav) = parse_sound(d, o, size) {
            if let Some(n) = name() {
                out.sounds.entry(n).or_insert(wav);
            }
        }
    }
    out
}

/// A chunk's own name: the first `u32 0x14` + 20-byte name in its first 120 bytes.
fn name_after(d: &[u8], start: usize, end: usize) -> Option<String> {
    let hay = &d[start..end.min(start + 120)];
    let j = hay.windows(4).position(|w| w == [0x14, 0, 0, 0])? + start;
    let raw = d.get(j + 4..j + 24)?;
    let raw = &raw[..raw.iter().position(|&b| b == 0).unwrap_or(20)];
    (!raw.is_empty() && raw.iter().all(|&c| c > 32 && c < 127)).then(|| String::from_utf8_lossy(raw).into_owned())
}

/// Owning objects keep `u32 0x14` + `char name[20]` shortly before their pointers.
fn name_near(d: &[u8], ptr: usize) -> Option<String> {
    let lo = ptr.saturating_sub(0x80);
    let mut j = ptr;
    while j > lo {
        j -= 1;
        if d[j..j + 4] == [0x14, 0, 0, 0] && j + 24 <= d.len() {
            let raw = &d[j + 4..j + 24];
            let end = raw.iter().position(|&b| b == 0).unwrap_or(20);
            let raw = &raw[..end];
            if !raw.is_empty() && raw.iter().all(|&c| c > 32 && c < 127) {
                return Some(String::from_utf8_lossy(raw).into_owned());
            }
        }
    }
    None
}

// ---- textures (8-bit paletted, GS-swizzled) ----

fn csm1(i: usize) -> usize {
    (i & 0xE7) | ((i & 0x08) << 1) | ((i & 0x10) >> 1)
}

fn unswizzle8(buf: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let block = (y & !0xF) * w + (x & !0xF) * 2;
            let swap = (((y + 2) >> 2) & 1) * 4;
            let py = (((y & !3) >> 1) + (y & 1)) & 7;
            let col = py * w * 2 + ((x + swap) & 7) * 4;
            let byte = ((y >> 1) & 1) + ((x >> 2) & 2);
            out[y * w + x] = buf[block + col + byte];
        }
    }
    out
}

fn parse_texture(d: &[u8], o: usize, size: usize) -> Option<Texture> {
    const HEADER: usize = 27;
    if size < HEADER + 1024 || d[o + 8..o + 12] != PTR {
        return None;
    }
    let fmt = d[o + 20];
    let w = u16::from_le_bytes([d[o + 21], d[o + 22]]) as usize;
    let h = u16::from_le_bytes([d[o + 23], d[o + 24]]) as usize;
    let mips = d[o + 25] as usize;
    if fmt != 1 || !(8..=1024).contains(&w) || !(8..=1024).contains(&h) || !(1..=8).contains(&mips) {
        return None;
    }
    let npix: usize = (0..mips).map(|m| (w >> m) * (h >> m)).sum();
    if HEADER + npix + 1024 + 1 != size {
        return None;
    }
    let mut pix = d[o + HEADER..o + HEADER + w * h].to_vec();
    let pal = &d[o + HEADER + npix..o + HEADER + npix + 1024];
    if w >= 16 && h >= 4 {
        pix = unswizzle8(&pix, w, h);
    }
    let mut rgba = Vec::with_capacity(w * h * 4);
    for &p in &pix {
        let j = 4 * csm1(p as usize);
        rgba.extend_from_slice(&[pal[j], pal[j + 1], pal[j + 2], (pal[j + 3] as u16 * 2).min(255) as u8]);
    }
    Some(Texture { width: w as u32, height: h as u32, rgba })
}

// ---- sounds (PS-ADPCM) ----

const RATES: [u32; 7] = [44100, 22050, 11025, 48000, 32000, 24000, 16000];
const COEF: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];

fn valid_frame(f: &[u8]) -> bool {
    (f[0] >> 4) <= 4 && (f[0] & 0xF) <= 12 && matches!(f[1], 0 | 1 | 2 | 3 | 4 | 6 | 7)
}

fn parse_sound(d: &[u8], o: usize, size: usize) -> Option<Vec<u8>> {
    if size < 64 || d[o + 8..o + 12] != [0x10, 0, 0, 0] {
        return None;
    }
    let rate = u32_at(d, o + 12);
    if !RATES.contains(&rate) {
        return None;
    }
    let end = o + size;
    let j = (o..end.saturating_sub(4)).rev().find(|&k| d[k..k + 4] == [0x07, 0x77, 0x77, 0x77])?;
    let last = j - 1;
    let mut start = last;
    while start >= o + 28 + 16 && valid_frame(&d[start - 16..start]) {
        start -= 16;
    }
    let pcm = decode_adpcm(&d[start..last + 16]);
    Some(wav(&pcm, rate))
}

fn decode_adpcm(a: &[u8]) -> Vec<i16> {
    let (mut h1, mut h2) = (0i32, 0i32);
    let mut out = Vec::with_capacity(a.len() / 16 * 28);
    for f in a.chunks_exact(16) {
        let shift = (f[0] & 0xF) as u32;
        let (c1, c2) = COEF[((f[0] >> 4) as usize).min(4)];
        for &b in &f[2..] {
            for nib in [b & 0xF, b >> 4] {
                let s = ((nib as i32) << 28) >> 16; // sign-extend nibble into the top of 16 bits
                let s = (s >> shift) + ((h1 * c1 + h2 * c2 + 32) >> 6);
                let s = s.clamp(-32768, 32767);
                h2 = h1;
                h1 = s;
                out.push(s as i16);
            }
        }
        if f[1] & 1 != 0 {
            break;
        }
    }
    out
}

fn wav(pcm: &[i16], rate: u32) -> Vec<u8> {
    let data_len = (pcm.len() * 2) as u32;
    let mut w = Vec::with_capacity(44 + data_len as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data_len).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        w.extend_from_slice(&s.to_le_bytes());
    }
    w
}
