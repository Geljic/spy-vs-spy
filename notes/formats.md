# Spy vs. Spy (PS2, PAL SLES_530.78) — file format notes

Clean-room notes from studying our own disc dump. Little-endian throughout.
Status legend: ✅ verified on real data · 🟡 partial · ❓ unknown
Class ids, PS2 loader per class and versions on the disc: [class-map.md](class-map.md) (+ `class-map.csv`).
Full class schema: vicious + our PS2 patches, see [vicious-ps2.md](vicious-ps2.md)

## Disc layout ✅

| File | Notes |
|------|-------|
| `SLES_530.78` | Main ELF. MIPS R5900, GCC C++, **stripped, no RTTI**. Strings are almost all SDK (network, DNAS, GameSpy). Little help for formats. |
| `DATA.PAK` | ~990 MB, all game content |
| `MODULES/`, `NETGUI/` | Sony IOP modules + network config GUI. Not game content. |

## DATA.PAK <a id="datapak"></a> ✅

- `0x000` u32 file count (741), u32 version (1); rest of sector zero
- `0x800` table of `count` × 48-byte entries:
  - `char name[40]`: NUL-terminated; bytes after NUL are uninitialised garbage
  - `u32 sector`: offset / 0x800
  - `u32 size`: bytes
- Data is sector-aligned, no compression, no overlaps.

Contents by extension:

| Ext | # | MB | What |
|-----|---|----|------|
| `.map` | 29 | 545 | Levels (container). Includes `MainMenu` |
| `.mov` | 17 | 306 | **Plain MPEG-2 program streams**; ffmpeg plays them |
| `.cin` | 102 | 102 | Cinematics (container) |
| `.mus` | 10 | 30 | Music ❓ (starts with repeated `0C 02 00 00` + zero rows; likely PS-ADPCM) |
| `.ls` | 10 | 4.6 | Loading screens (container) |
| `.loc` | 528 | 4 | Localised text per language (`eng/fre/ger/spa`) (container) |
| `.gam` | 1 | 0.6 | Global game definition "SpyVsSpy" (container) |

Filenames are 64-bit hex IDs (e.g. `Maps/0439519CAF617032.map`). The same ID
appears inside the file's own header at 0x28 as two u32 (lo word first), and
assets reference each other by these IDs.

### Maps

| Classic MP arenas | Story | Other |
|---|---|---|
| HauntedHouse, MedievalMansion, VolcanicIsland, CreepyCarnival, Area51, OilRig, SpaceStation, RobotFactory | *_Story variants (Mansion, SStation, KC, Area51, HHouse, Volcano, OilRig, RF) | MainMenu, Embassy_Room_05…20 |

The map's display name is the 20-byte string at file offset 0x34.

## Container (chunked object stream) <a id="container"></a> ✅

Used by `.map .gam .ls .cin .loc`. Read sequentially by the engine's stream class
(loader `FUN_0013cfc0`, chunk begin `FUN_0013d1f0`, chunk end `FUN_0013d288`,
byte read `FUN_00141fc0(file, dst, n)`):

- `00` `AF FA AF FA` magic (`FE AF FE AF`-style byte-swapped variant also accepted), `04` u32 version = 3
- **Chunk begin** (position first aligned up to 4): `BB BB BB BB`, u32 **absolute end offset**, u32 **class serialization version**
- **Chunk end**: the engine seeks to `end` and reads the guard `BE BE BE BE`
- File ends `EF FE EF FE`
- Chunks nest: a class's chunk usually starts with its base class's chunk, then its own fields, then child objects' chunks.
- Fields inside are **byte-packed little-endian, no alignment** (the engine reads them one by one).
- (Earlier notes called these "pointers" with a "count/tag"; that was wrong: offset = chunk end, tag = version.)
- Named objects carry `u32 0x14` + `char name[20]`. `tools/container.py: name_near()` still works for labelling.
- The engine also has a separate **bit-packed** stream (`FUN_0013b418` uint, `FUN_0013b210` int,
  `FUN_0013b690` fixed-point float, 1-bit 0/1 shortcuts). Only ~60 functions use it: almost certainly
  **network packets**, useful for online play later.

### Game logic lives in the data 🟡

Map strings are dominated by engine-level behaviour names, e.g. `HL_AISpyBase`,
`LL_Spy`, `HL_SpyHuman`, `State`, `VarPlayer`, `MsgBiped`, `RelPlayer`,
`CatItemTypes`, `EnumGameMode`, `GsMultiplayer`, `Net_ScoresAndLobby`,
`DG_*` (menu screens), `Anim*` (animation sets). Prefixes look like:
`HL_/LL_` high/low-level behaviours, `Var` variables, `Msg` messages,
`Rel` relations, `Cat` categories, `Enum` enums, `Const` constants, `DG_` dialogs.
So the rules are likely a data-driven behaviour system (Vicious Engine), which
an interpreter could run. That needs much more study.

## Texture <a id="texture"></a> ✅ (8-bit paletted)

An allocation laid out as:

⚠ see vicious-review.md §6.3: offset 8 is a chunk header (not a pointer) and offset 20 is an `enabled` flag; the real format byte is in the `Texture` base before the guard.

| Off | Size | Field |
|-----|------|-------|
| 0 | 4 | guard `BE BE BE BE` |
| 4 | 1 | has-mips flag (1/0) |
| 5 | 3 | unknown (`05/ff/02/07/...`, `00 00`) |
| 8 | 12 | pointer (to small owned object) |
| 20 | 1 | format: `1` = PSMT8 (8-bit indexed) |
| 21 | 2 | width |
| 23 | 2 | height |
| 25 | 1 | mip level count |
| 26 | 1 | pad |
| 27 | Σ(w>>m)(h>>m) | pixels, base level first |
| … | 1024 | palette: 256 × RGBA, alpha 0..0x80 (×2 for 8-bit) |
| … | 1 | trailing byte |

- Pixels are **GS-swizzled PSMT8**; unswizzle with the standard PSMT8→PSMT32 mapping (`tools/tex.py`).
- Palette is in **CSM1 order** (swap bits 3 and 4 of the index).
- Seen sizes: 32², 64², 128², 256². Name comes from the owning material object
  (`cas1_glasses`, `256cc_w_spy`, ...). Fonts are white RGB with the glyphs in alpha.
- Verified: Creepy Carnival → 270 textures, all correct visually.
- Not yet seen: 4-bit (PSMT4) or true-colour formats. Format byte ≠ 1 is skipped.

## Sound 🟡

Allocations with a u32 `0xAC44` (44100 Hz) and float `1.0` (volume?) look like
sample headers, e.g. `bf 0c 00 00 | 10 00 00 00 | 44 ac 00 00 | 00 00 80 3f`.
Probably PS-ADPCM bodies. Not decoded yet.

## Collision geometry <a id="collision"></a> ✅

Version-4 chunk. Loader `FUN_00298360` → base `FUN_002976c0` (v1 chunk holding one u32) → `FUN_00298728`.

```
u8   present
u32  ?, u32 ? (float-ish)
u32  n_nodes;  n_nodes × { u16, u16, float3 min, float3 max }      (BVH, boxes padded by 0.01 at load)
u32  n_leaves; n_leaves × { u8, u8 n_polys, u32 (low 4 bits = n_normals-1), u32 (bit31 flag),
                           u32 (low 27 bits = offset), int16x3 lo, int16x3 hi }
u32  poly-data size
per leaf:
  n_polys × u16[5] (u16[4] if chunk version ≤ 3):
      [0] bit0 = closes a triangle with the previous two descriptors (strip)
      [1] bit0 = flip winding
      [2] 6-bit surface type
      [3] 4-bit normal index
      [4] 4-bit vertex index (into this leaf's vertices)
  n_normals × int16x3   unit normals (×16384)
  (max vertex index + 1) × int16x3 vertices (world space, z-up; ×0.01 is provisional)
```
Verified: every map parses exactly to its chunk end. All 29 maps export (`tools/collision.py`):
e.g. CreepyCarnival = 70 objects / 13,356 triangles, and the 8 arenas render recognisably.

## Animation clip ✅ (layout) / 🟡 (playback)

Version-3 chunk, loader `FUN_001fd368`, base `FUN_001410f0`:
```
u32 frame_ms, u32 duration_ms, u16 n_frames, i16 n_tracks
u8 flags[4]           (flags[3]: has per-frame u32 table)
u32 n_rot, u32 n_pos, u32 n_scl, float pos_scale, float scl_scale
u8   track_channels[n_tracks]
i16x4 rot_keys[n_rot]   quaternions ×16384
i16x3 pos_keys[n_pos]   × pos_scale
i16x3 scl_keys[n_scl]   × scl_scale
u32 n; u16[n]  ×3      key index tables
if flags[3]: u32[n_frames]
u32 n; u8[n]
u32 n; u16[n]
```
181 clips in CreepyCarnival. Needs the skeleton format before it can drive models.

## Level objects, render meshes and materials <a id="render-mesh"></a> ✅

See tools/levelmesh.py and game/src/model.rs for the exact reader.

Level object = **v3 chunk**: `[v1 name] [v4 geometry] [v1 material list] [v1 surface refs]`.
Geometry (v4, loader `FUN_00236e38`): base, LODs (node tree), **render data** (`FUN_0025ce78`), u8 + collision.

Render data (v1): `u8 present`, 4 × `{u32 size, bytes}` = positions int16x3 (×0.006 m), normals int16x3 /4096,
UVs int16x2 /4096, strip indices u8 (bit 7 = restart); then `u32 n_groups` × `{u16 material slot (|0x8000 flag),
u32, u32 n_sub, u32, n_sub × {u32 n_verts ≤76, float3, float radius, u32 n_lists, u32 len[n_lists]}}`.
Per-batch blob slices and each index list are padded to 16 bytes. Verified exact on every object.

Material list: `[v1 name] u32 n`, n × object ref `{u16 type 0x1b, u16, u64 id, u32, u32 0x14, name[20], u16, u16}`
+ `u8 inline` (+ v5 material chunk whose v3 child is the texture). Draw code: `obj.materials[slot & 0x7fff]`.
`h_skybox*` objects are sky domes. Props (`ladder`, `plant_potted`, ...) sit at the origin: **placement not found yet**.
Templates (v9): `[v2 name] [v6 skinned/hierarchical model, bone matrices via FUN_00208db8] [v1 materials]`.

## Entities (placed gameplay objects) <a id="entities"></a> ✅ (transform) / 🟡 (classes)

Version-13 chunk, loader `FUN_0015a848`:
```
[v1 name chunk]
object ref  (class/definition, e.g. Door_MM, Safe_MM_Monitor, Spy_HumanPlayer, AISpy_SinglePlayer)
u8
object ref  (version > 11; usually null)
u8, u8, u8, u32, u32 (v>6), u32 (v>10)
float3 position (file units /100 → ×0.6 = metres, z-up), float3 X axis, float3 Y axis, float3 Z axis
... child lists (FUN_0013ada8, refs)
```
**Object reference** (`FUN_0013e0a8`): `i16 type, u16 flags`; type < 0 = null (4 bytes). Otherwise `u64 id`,
`u32` if flags ≠ 0 and bit 15 clear, string (`u32 len, chars, u32`), `u8 inline` + definition chunk if set.
Mansion_Story has 124 entities: Player, AI_Spy_White, 16 safes, 12 doors, laser gates, keypads,
chandeliers, bridges, goal rooms, sound entities, spawners, the boss. Positions verified on a map overlay.
