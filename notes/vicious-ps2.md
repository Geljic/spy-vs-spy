# vicious on the PS2 disc: what differs from Xbox

[vicious](https://github.com/vicious-rebirth/vicious) (MIT, Alexandre Lavoie) is a clean-room schema of the
Vicious Engine formats, built from the **Xbox** Spy vs. Spy. This page lists every place where the **PS2 PAL**
files (SLES_530.78) differ from that schema, as found by decoding all of them. The fixes are a patch series
in [`tools/vicious-ps2/`](../tools/vicious-ps2/) (6 patches against vicious commit `1135220`).

Status legend as in [formats.md](formats.md): ✅ verified on our PS2 data (command + result given) ·
🟡 partial / inferred · ❓ unknown.

## Result ✅

Every container file on the disc decodes to the byte with the patched `vcs2json`:

```sh
tools/vicious-ps2/setup.sh      # clone vicious @1135220, apply patches, codegen, Release build
tools/vicious-ps2/check.sh      # vcs2json --strict --ps2 over work/x, table per extension
```

| ext | files | pass (strict) | before the patches |
|-----|------:|------:|------|
| `.map` | 29 | 29 | 29 crash |
| `.cin` | 102 | 102 | 67 crash, 1 bad header, 34 rc 0 |
| `.ls` | 10 | 10 | 6 crash, 4 bad header |
| `.gam` | 1 | 1 | crash |
| `.loc` | 568 | 568 | 568 rc 0 (no strict check existed) |
| **total** | **710** | **710** | |

"Strict" (`vcs2json --strict`, patch 0001/0002) means: every chunk's data ends exactly at the end offset in
its `BB BB BB BB` header (after 4-byte alignment, where the `BE BE BE BE` guard is), the file ends right
after `EF FE EF FE`, and no error was logged. The first failure stops the run and prints the chunk stack.
No part of a PS2 file is skipped as opaque bytes: the collision tree, which vicious keeps as a raw blob for
Xbox, is decoded field by field on PS2 (patch 0004).

The `.mov` files are MPEG-2 and `Sounds/*.mus` are not containers (see [Music](#music)); neither is checked.

## How the PS2 switch works

- Schema DSL gets `ctx.platform()` (0 = Xbox, 1 = PS2). Codegen emits it as a global `viciousPlatform`
  (C) / `viciousPlatform` export (JS). `stdDecoder()` sets it from `VICIOUS_PLATFORM=ps2`; `vcs2json --ps2`
  does the same. Every PS2 change below is behind `platform() == 1`, so the Xbox code paths are unchanged
  (we have no Xbox data to re-test them: 🟡 for Xbox, by construction only).
- Extra `vcs2json` options: `--strict`, `--ps2`, `--types` (adds `__class` / `__id` to every class object in
  the JSON), `--stats` (prints `CHUNK name version count` lines).
- Lists with no count in the file (PS2 group tables that run to the chunk end) are read with a helper in
  `model.ts` (`readGroupsToChunkEnd`): the decoder loops until the chunk end and stores the count for the
  visitor/encoder.

## Differences, class by class

Versions and counts are over the whole disc (`vcs2json --ps2 --stats`, summed over all 710 files).
"Chunk" = a versioned `BB BB BB BB` block; structs without an `__id` are chunks inside their class.

### Texture (`__id` 10) v2, ImageTexture (`__id` 12) v3, ImageTextureBuffer chunk v1 ✅

7707 Texture / 7518 ImageTexture chunks. Pixel layout: [formats.md#texture](formats.md#texture).

| Where | Xbox schema | PS2 |
|---|---|---|
| Texture, after `height` | nothing | `u8 ps2Flags` (always 0 in the focus maps) |
| ImageTextureBuffer, after `enabled` | `u32 levels` | `u16 width`, `u16 height` (repeat of Texture's), `u8 levels`, `u8` (0) |
| pixels | size depends on `Texture.format` (DXT1 = ½ byte/pixel) | always 1 byte/pixel (PSMT8, GS-swizzled), Σ over levels of w·h |
| palette | only when `format == 3` | always: 256 × RGBA, CSM1 order, alpha 0..0x80 |
| after palette | nothing | `u8` (0) |

- `Texture.format` is 0, 1, 2 or 3 on PS2 (counts in the two focus maps + .gam: 1 = 275, 3 = 122, 0 = 81,
  2 = 23) but the pixel data is 8-bit indexed for every value; what the byte means on PS2 is ❓.
- Our older note ("byte 20 = format 1 = PSMT8, 12-byte pointer, has-mips flag") was reading these fields:
  byte 20 is `ImageTextureBuffer.enabled`, the "pointer" is the buffer's chunk header, the "has-mips flag"
  is `ImageTexture.f_0x48`.
- Verified: ps2Width/ps2Height equal Texture width/height in all 502 textures of the focus maps + .gam;
  `txr2png` (patch 0005) renders CreepyCarnival's textures correctly (contact sheet checked by eye).

### Sound (`__id` 38) v2, SoundBuffer chunk v1 ✅ layout / 🟡 meaning

3575 chunks. Earlier notes: formats.md, "Sound" section.

| Where | Xbox schema | PS2 |
|---|---|---|
| SoundBuffer after `enabled`, `sampleRate` | `WAVFmtChunk` (if not disabled) | `f32` (0.5714 = 4/7 in all 275 seen), `u32 blockCount` |
| data | `u32 size` + WAV (IMA ADPCM) bytes | `blockCount × 16` bytes of mono PS-ADPCM, only if the Sound is not disabled |

- The `f32` and `blockCount` are present even when `Sound.disabled` = 1; those sounds have no inline data
  and are **streamed music**: their `blockCount` equals the size/16 of a `Sounds/*.mus` file and their label
  names it (`lm_mf001_001` … `lm_mf002_004`, 10 sounds ↔ all 10 `.mus` files, in both focus maps) ✅.
- Inline sound effects say `SoundBuffer.sampleRate` = 32000 while `Sound.samplingRate` = 44100 (255 of 255).
  `snd2wav` uses 32000 🟡 (not listened to). Streamed music says 44100 in both.
- `Sound.duration` does not match samples/rate for either rate ❓.

### StaticMesh (`__id` 20) v4, render data chunk `V20_5` v1 ✅

686 chunks. The Xbox body (`V20_6`: sections + vertex buffer + u16 index buffer) is replaced by
`PS2StaticMeshBuffers`: 4 blobs (`u32 size` + bytes: positions int16x3, normals int16x3, UVs int16x2, strip
indices u8) + `u32 groupCount` + groups `{u16 material, u32, u32 batchCount, u32, batches {u32 vertexCount,
float3, f32, u32 stripCount, u32 stripLengths[]}}`. Same as [formats.md#render-mesh](formats.md#render-mesh).
Verified on every static mesh of both focus maps (69): blob sizes = Σ per-batch slices padded to 16 bytes
(positions/normals a16(6v), UVs a16(4v), strips Σ a16(stripLength)).

### Collision: `V20_9` chunk v4 with `V20_11` tree ✅

1057 chunks. vicious reads the tree as raw bytes (its `V20_11` classes are a TODO). On PS2 we decode it with
`V20_11` after fixing two bit-field bugs: the leaf's low-27-bit merge wiped the normal count, and the 4-bit
field stores *count − 1*. Matches [formats.md#collision](formats.md#collision); all 1057 trees end exactly at
their chunk end. (The two fixes are generic, but only the PS2 path uses `V20_11`.)

### Model (`__id` 29) v1, VertexBuffer chunk v1 ✅

1676 chunks (static instances, sky domes). Xbox: `enabled` + raw vertex buffer. PS2:

```
bool enabled
AssetReference mesh          the StaticMesh these colours belong to
u32 size + bytes             one 16-bit colour per vertex, sliced per batch, slices padded to 16
groups ...                   copy of the mesh's group/batch table, NO count: runs to the chunk end
```
Verified: colour size = Σ a16(2·vertexCount) over the copied batches for all 368 instances in the focus maps.
Colour bit layout (likely 1-5-5-5) 🟡.

### DynamicModel (`__id` 45) v2, DynamicModelVertexData chunk v1 ✅

2844 chunks. Same change as Model: Xbox raw buffer → PS2 colours + group table to the chunk end.

### DynamicMesh (`__id` 28) v6, DynamicMeshBody chunk v1 ✅ layout / 🟡 skinned vertex

This is the **v6 entity model** (doors, safes, the spy) that blocked the README's step 1. 2539 chunks; the
`DynamicMesh` fields before the body (helper points, transforms, hit boxes, collision) already match vicious.
The Xbox body (32 `MeshSection`s + rigged/static vertex buffers + index buffer) is replaced by:

```
bool enabled
part rigid:   u32 groupCount, u8 sectionCounts[32],
              if groupCount: 4 blobs (u32 size + bytes) = positions int16x3, normals int16x3, UVs int16x2, strips u8
                             then for section i = 0..31: sectionCounts[i] × static-mesh group (with u16 material)
part skinned: u32 groupCount, u8 sectionCounts[32],
              if groupCount: u32 quadwords + quadwords×16 bytes = vertices, 16 bytes each
                             u32 size + UVs int16x2, u32 size + strips u8
                             then for section i: sectionCounts[i] × group {u32, u32 batchCount, u32, batches} (no material)
32 × { u32 n, n × SkinMeshFrame }   (same 8-byte-record frame lists as Xbox)
```
- "Section" = one of 32 joint/matrix slots: rigid groups move with their slot (vicious' Xbox "static ranges"),
  skinned vertices blend joints (Xbox "rigged" buffers). Naming 🟡.
- Verified on all 172 rigid and 18 skinned parts in the focus maps: rigid blob sizes = a16(6v)/a16(6v)/a16(4v)
  and Σ a16(strip); skinned quadwords = vertex count exactly, UV and strip blob sizes match the batch tables.
- Skinned vertex (16 bytes): many sampled records end in `u8 joint0, u8 joint1, u16 weight0 ×4096`
  (e.g. `01 00 00 10` = joint 1, weight 1.0), the first 12 bytes look like 6 × int16; but only ~50–95 % of
  records fit that reading, so the layout is **not** settled 🟡. Next step for the model work.
- JSON: an entity template's model is `EntityTemplate.modelTemplate.asset.mesh.asset` (`ModelTemplate`
  `__id` 44 → `DynamicMesh`), skins in `ModelTemplate.skins`, the body in `DynamicMesh.body` with
  `ps2Rigid`, `ps2Skinned`, `ps2Frames`.

### Model material shader chunks (empty on PS2) ✅

`SingleTextureModelMaterialShader` (in class 27, 5784×), `DualTextureModelMaterialShader` (class 73, 561×)
and `V314_1` (class 314, 87×) hold an `AssetFromType` on Xbox; on PS2 the chunk is empty (12-byte header,
guard). Made Xbox-only.

### Sky (`__id` 473) v3 ✅

34 chunks. After vicious' fields PS2 has a typed asset list (`u32 count`, `count × {i32 type, chunk}`): the
sky dome `Model` instances (3 in CreepyCarnival). Gated on PS2; class-map (`notes/class-map.csv`) shows the
PS2 loader gates it on version > 2, so it may be a v3 field rather than a platform one 🟡.

### V248 (`__id` 248) v10 ✅ layout / ❓ meaning

65 chunks, a script statement that vicious leaves empty (`__todo`). PS2 v10: base `V368`, `u32`, then a wrapped
typed asset (`AssetFromTypeWrap`, usually −1). Gated on PS2.

### Not a PS2 difference, but needed to read PS2 maps

- Arena full: vcs2json's 32 MB arena is too small for 20 MB maps; `stdDecoder` now falls back to the heap.
- JSON writer: a list right after a list was nested inside it and the node stack ran past 255 on deep script
  trees (wrapping a `uint8_t`); fixed arrays of atoms (`u8[32]`) were written as `{}`. Fixed in patch 0002.

### class-map cross-check

`notes/class-map.csv` (from the ELF loaders) flags ~50 classes whose PS2 loader has version gates vicious
does not. Apart from Sky and V248 above, none of them changed how the PS2 bytes decode: the strict run
passes for all 710 files, so either the gated fields are absent at the versions on disc or the extra gates
read nothing. Open as 🟡 for versions not present on the disc.

## Where things are in the JSON

JSON dumps (`vcs2json --ps2 --strict --types`), in `work/ps2-decode/json/` (gitignored):
`Mansion_Story.4DC0D0DAD36EB8EF.map.json`, `CreepyCarnival.5C35DB16C6D45BCC.map.json`,
`SpyVsSpy.0000000100000001.gam.json`.

**References are inline only the first time.** An `AssetReference` has `type` (= class id), `id` (64-bit hex)
and `first`; the object itself (`asset`) is written where `first` is true, later references carry only the
id. Index all objects by `id` before following references.

Map root: `content.asset` (`Map`). Its `groups.list[i].object` (same order in both focus maps):

| i | group | contents (Mansion_Story / CreepyCarnival) |
|---|---|---|
| 7 | StaticInstanceGroup | `StaticInstance` 29 / 232 → `.model` (Model) → `.f_0x40` StaticModelTemplate → `.mesh` StaticMesh |
| 13 | PointGroup | `Point` 145 / 101 |
| 14 | TriggerGroup | `Trigger` 98 / 70: `onEnter`, `onInside`, `onExit` scripts, `boundingBox` |
| 16 | EntityGroup | `Entity` 70 / 44: `template_` (EntityTemplate), `transform`, `body` |
| 17 | StaticLightGroup | **map lights**: Point 247 / 259, Spot 0 / 22, Directional 4 / 1, Area 15 / 1, Sky 1 / 2; fields `intensity`, `color` (packed i32), `transform`; directional adds `azimuth`, `elevation` |
| 22 | NavMeshGroup | `NavMesh` 18 / 1 |
| 34 | SkyGroup | `Sky` (with `ps2Models`) |

- **Entity models (v6)**: `Entity.template_` → `EntityTemplate.modelTemplate` → `ModelTemplate.mesh`
  (`DynamicMesh`, body as above). Templates are usually first written inside a script that spawns them, so
  look them up by id.
- **Behaviour scripts**: `Script` (78,919 chunks on the disc) objects under `onEnter`/`onExit`/… of Triggers,
  `Map.onMapStarted` etc., entity `f_0x18b` lists; `EntityTemplate.f_0x60` → `StateMachine` (3022 on disc, v3)
  → states with `Script`s; `Function` objects (`function.asset`) for called functions. Statement/expression
  classes are named by vicious where known (`IfStatement`, `CallFunctionStatement`, `EntitySelector`, …).
- `.gam`: `content.asset` is `Game` with global groups (Functions, Variables, Dialogs, StateMachines,
  ExternalVariables 58) and `onGameStarted`, `onMapStarted`, … scripts.

## Other vicious tools on PS2 data

Tested with `VICIOUS_PLATFORM=ps2` on CreepyCarnival (unpacked with `unpack` into `work/ps2-decode/tools-test/`).

| Tool | Status | Notes |
|---|---|---|
| `vcs2json` | ✅ 710/710 strict | patches 0001–0004, 0006 |
| `unpack` | ✅ works | 3854 asset files from CreepyCarnival; re-encoded `.txr/.msh/.snd/.anm/.map` decode strictly (sample of 40 each). Always exits 1 (upstream: `return 1` at the end of `main`) |
| `txr2png` | ✅ 270/274 | patch 0005 (PSMT8 unswizzle + CSM1); the 4 others are render targets / frame buffer textures |
| `snd2wav` | ✅ 138/138 rc 0 | patch 0005 (PS-ADPCM → PCM); 128 WAVs, 10 empty (streamed music). Rate 32000 🟡 |
| `mus2wav` | ✅ 10/10 | patch 0005, see Music; output in `work/ps2-decode/mus/` |
| `anm2gltf` | 🟡 rc 0 | non-empty glTF for 20/20; animation layout matches ours, output not inspected |
| `msh2gltf` | ❌ geometry | rc 0, writes skeleton/skin/materials but no mesh: it reads the Xbox buffers. Needs a PS2 strip → triangle path (not cheap) |
| `ms2gltf` | 🟡 rc 0 | not inspected |
| `mtl2gltf` | ❌ | 0-byte output for PS2 `.mtl`, not investigated |

### Music <a id="music"></a> ✅ layout / 🟡 rate

`Sounds/*.mus` on PS2 has **no header**: the whole file is mono PS-ADPCM, 16-byte blocks, flags byte 2 on every
block (the repeated `0C 02 00 00` + zero rows in formats.md are silent blocks). The Xbox `.mus` (WAV-style
header + IMA ADPCM) does not apply.
- Mono, not interleaved stereo: the decoded signal is smooth (lag-1 autocorrelation 0.99) and splitting it at
  any interleave (16 B … 32 KB) gives no left/right correlation beyond what adjacent blocks have.
- 44100 Hz: from the streamed `Sound` that names each file (see Sound above). Lengths 71.6–139.7 s.
- `VICIOUS_PLATFORM=ps2 mus2wav file.mus out.wav [rate]`. Not listened to.

## Open

- Skinned vertex layout (16 bytes) and joint/section → skeleton mapping (helper points, `Skin`).
- What `Texture.format`, the PS2 texture/buffer padding bytes, the SoundBuffer `f32` (4/7) and `Sound.duration` mean.
- Colour format of the per-instance vertex colours.
- `msh2gltf` / `mtl2gltf` for PS2.
