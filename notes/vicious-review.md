# Review: vicious (Vicious Engine data formats), against our PS2 work

Reviewed 2026-10-12, commit `1135220` (2026-10-03). Repo: github.com/vicious-rebirth/vicious, 50 commits from
2025-12-18 to 2026-10-03, all by Alexandre Lavoie. Local clone: `work/vicious` (read only). Scratch scripts and
outputs for every number below: `work/review/`.

Legend: ✅ verified on our PS2 data (the command is in §9) · 🟡 likely, partly checked · ❓ open.

## 1. Short version

- vicious is a **clean-room, schema-driven codec** for Vicious Engine asset files, built on the **Xbox** release of
  Spy vs. Spy. One TypeScript schema (518 classes) generates a C decoder, encoder, visitor and free routines,
  plus a JS codec. Small CLI tools convert textures, meshes, animations, materials and audio to PNG/DDS/glTF/WAV.
- The **container, object model, references, entities, lights, animation and mesh skeleton layouts match our PS2
  data**. Every class id and field we checked lines up (§6). The PS2-specific parts are the bulk-data buffers
  (pixels, vertex/strip data, sound samples, collision), where vicious has the Xbox layouts.
- Its biggest value to us is the **names**: class ids → class names, and the field order of the behaviour
  system (scripts, statements, expressions, state machines). With those names our PS2 data turns out to hold the
  whole game logic as state machines and scripts. Door logic, for example, is a `Door` state machine with five
  states (`Closed`, `Closing`, `Open`, `Opening`, `Locked`) and seven local functions (§7.1,
  [script-vm-plan.md](script-vm-plan.md)).
- 69% of fields and 56% of classes are still unnamed (`f_0x..`, `V<id>`). The October "AI class/field renaming"
  commit only renamed things; it changed no layouts. Treat its names as educated guesses (§4.4).
- We found **one definite error in our own notes** (the texture header, §6.3) and several places where vicious
  adds information we lacked: the file header's class type, prop placement, the light classes, skeleton
  field names and animation track flags.

## 2. What it is and how it works

| Part | Where | What it does |
|---|---|---|
| Schema DSL | `packages/core/src/schema/core.ts` | `Class` (has `__id`, chunked), `Struct` (inline, optionally chunked via `__metadata`), `Atom` (U8…F32). Fields are `field(Type, {condition, custom})`; conditions are expressions over `ctx.version()`, so one schema covers every serialization version. `__offset` is the address of the loader in the **Xbox XBE**; `__todo` marks unfinished classes. |
| Types | `packages/core/src/schema/types/*.ts` | 493 files (+ `index.ts` = the "494"). They define **518 classes, ids 1–518 with no gaps**, 105 structs and 9 atoms. |
| Codegen | `packages/c/codegen/{decoder,encoder,visitor,free,type,emit}.ts`, `packages/js/codegen` | Walks the schema and emits `packages/c/src/generated/{decoder,encoder,visitor,free,util}.c` (≈47k lines) and a TS codec. Class dispatch is one big `switch (typeId)` (`decodeType`). |
| C runtime | `packages/c/src/{context,memory}.c`, `include/vicious/*.h` | Arena/pool allocators, std file decoder/encoder contexts. |
| CLI apps | `apps/*/src/*.c` | `vcs2json` (any asset → JSON), `pack`/`unpack` (split a `.map`/`.gam` into per-asset files and back), `pakdump` (DATA.PAK), `txr2png`/`txr2dds`/`dds2txr`, `msh2gltf`, `mtl2gltf`/`ms2gltf`, `anm2gltf`, `snd2wav`/`wav2snd`/`mus2wav`/`wav2mus`/`wav2xbox`, `test` (round trip). `edit` is an ImGui/GLFW GUI field editor. |
| Web | `apps/*/src/*.html`, `packages/docker/build/web/Dockerfile` | Emscripten builds of the tools for vicious-rebirth.github.io. |
| Docker / CI | `packages/docker/build/alpine/Dockerfile`, `.github/workflows/release.yml` | Alpine image builds all CLI tools (runs `pnpm codegen` correctly). CI only builds Alpine + web on a tag and makes a draft release. **No tests run in CI.** |

How decoding works: a file is `AFFAAFFA`, `u32 version`, then one chunk holding the `AssetFileHeader` and the
asset, then `EFFEEFFE` (`file.ts`). **Every level of a class's inheritance chain is its own chunk**
(`BBBBBBBB`, `u32 end`, `u32 version` … `BEBEBEBE`), except classes marked `__metadata = false` (`Base`,
`Expression`, `TypedAssetSelector`), which have no chunk. Polymorphic children come in two forms:
`AssetFromType` = `i32 classId` + the object (or `-1`), and `AssetReference` = our "object reference" (§6.9).

## 3. Coverage

Classes grouped by area (our grouping, by inheritance and name; borderline classes are assigned by hand, so
treat the row split as approximate). "Own fields" excludes `base` and deprecated slots.

| Area | Classes | Named classes | `__todo` classes | Own fields | Unnamed fields (`f_…`) |
|---|---:|---:|---:|---:|---:|
| Script: statements | 90 | 31 | 10 | 390 | 311 (79%) |
| Script: expressions | 79 | 37 | 6 | 178 | 104 (58%) |
| Script: state machines, enums, variables, functions, handlers | 45 | 11 | 3 | 92 | 65 (70%) |
| UI, dialogs, fonts, sprites, text | 45 | 10 | 26 | 104 | 77 (74%) |
| Empty placeholders (no base, no fields) | 46 | 0 | 0 | 0 | 0 |
| Groups (per-type folders, e.g. `TextureGroup`) | 36 | 36 | 0 | 1 | 0 |
| Materials, surfaces, skins | 35 | 19 | 13 | 64 | 40 (62%) |
| Meshes, models, geometry, nav | 25 | 18 | 7 | 71 | 46 (64%) |
| Selectors & data sources | 24 | 16 | 3 | 35 | 7 (20%) |
| Effects (particles, trails, projectors, flares, sky) | 23 | 12 | 8 | 114 | 109 (95%) |
| Entity components & animation (mostly unnamed) | 19 | 2 | 4 | 57 | 36 (63%) |
| Container, metadata, top-level files (`Game`, `Map`, `World`, …) | 13 | 10 | 1 | 84 | 32 (38%) |
| Cameras & cinematics | 11 | 5 | 1 | 65 | 58 (89%) |
| Textures | 10 | 4 | 1 | 28 | 14 (50%) |
| Entities & world objects (`Entity`, `EntityTemplate`, `Trigger`, `Path`, `Point`, `Timer`) | 7 | 6 | 1 | 54 | 34 (62%) |
| Lights | 6 | 6 | 0 | 11 | 3 (27%) |
| Audio | 3 | 3 | 0 | 11 | 3 (27%) |
| Other | 1 | 0 | 1 | 0 | 0 |
| **Total** | **518** | **226** | **85** | **1359** | **939 (69%)** |

Notes:
- Localisation (`.loc`) is a separate top-level struct, `LocalizationFile` (`localization.ts`), not a class.
- The 46 empty placeholders (`V6`, `V7`, `V98`, …) are ids the Xbox data never uses; the schema only keeps their
  numbers.
- `__todo` classes with no fields decode only if the object really has no own fields. Several of these are
  **used on PS2**, e.g. the whole heightmap material family (#9, #81, #82, #84, #85, #95) and `ParticleSystem`
  (#48) 🟡 (class-id scan, §9).
- Mesh collision (`V20_9`) is read as an opaque byte buffer. The decoder for it is commented out in
  `staticMesh.ts`.

## 4. Quality and gaps

### 4.1 What's solid
- The design: one versioned schema, generated codecs, and an encoder that round-trips (`apps/test-c`). The README
  says all Xbox `.gam/.map/.ls/.cin/.mus/.loc` decode and re-encode.
- The core object model (`Base → Named → Object`, groups, `AssetReference`, `AssetFromType`, `Label`) matches the
  PS2 data byte for byte (§6).
- Converters exist for every main asset type, which documents the semantics of the buffers: track flags in
  `anm2gltf.c`, skeleton building in `msh2gltf.c`, palette/mip handling in `txr2*`.

### 4.2 What's unfinished
- **69% of fields are unnamed**, 85 classes are `__todo`, and 292 of 518 classes are only `V<id>`.
- The commit log flags these converters as incomplete: `msh2gltf` ("not fully tested normals", 2026-01-31),
  `ms2gltf` ("untested merging", 2026-01-24), `mtl2gltf` ("basic conversion").
- The `pak` README documents a `pakbuild` tool that does not exist; only `pakdump` does. `pakdump` also writes to
  the path taken from the archive without sanitising it, so a crafted PAK could write outside the output folder.
- **The decoder ignores the chunk `end` offset.** `decodeMetadataFooter` aligns to 4 bytes and checks for
  `BEBEBEBE`; on a mismatch it only logs "bad footer magic" and carries on (`packages/c/src/generated/decoder.c`).
  Any layout difference (a PS2 version, a `__todo` class) therefore derails the whole read. This is the likely
  cause of the PS2 segfaults; the engine itself seeks to `end`. A tolerant mode that seeks to `end` would make
  vicious decode PS2 files with partial coverage.
- The root of the repo has **no LICENSE file**. "MIT" appears only in each package's `package.json`.

### 4.3 Build issues we hit
| Issue | Detail |
|---|---|
| README omits codegen | `packages/c/src/generated/` is gitignored (`packages/c/.gitignore`) and nothing runs codegen on install, so `pnpm setup:release` fails until `pnpm codegen` has run. Working order: `pnpm install --ignore-scripts && pnpm codegen && pnpm setup:release && pnpm build:exe --concurrency=1 --continue`. The Docker files do run codegen. |
| `apps/edit` fails | Our CMake cache had been created under a different path (the clone was moved), so we reconfigured from scratch in the copy `work/review/vicious` using the already-fetched imgui/cimgui sources. The **real error is at configure time**: `None of the required 'glfw3' found` and `Could NOT find OpenGL (missing: OPENGL_opengl_LIBRARY OPENGL_glx_LIBRARY OPENGL_INCLUDE_DIR)`. This machine has the `glfw` runtime but not `glfw-devel`, `libglvnd-devel` or `mesa-libGL-devel`. The README lists OpenGL as optional and doesn't mention GLFW. This is not a code bug: installing those dev packages should fix it (not tried; no system changes made). |

### 4.4 The "AI class/field renaming" commit (`d621a56`, 2026-10-03)
We parsed the schema before and after the commit and compared the two by class id (`work/review/before.json`
and `schema.json`):
- **132 classes renamed** (98 → 226 named classes), **185 fields renamed**, **0 layout changes**. Field types and
  order are identical once the renamed helper types are mapped across. `__todo` count unchanged (85). One
  follow-up commit fixed a renamed identifier in `mtl2gltf.c`.
- So the commit changes names only, and decoding is unaffected. The **names are guesses**, presumably from the
  Xbox loader code and field usage. Trust them roughly in this order:
  1. Names that a field's referenced class confirms. On PS2, `CallFunctionStatement.function` points at a
     `Function` in 11,062 of 11,063 uses ✅.
  2. Structural names (`IfStatement`, `WhileStatement`, `SwitchStatement`/`CaseStatement`,
     `BinaryOperationExpression`), which follow from the field shapes 🟡.
  3. Purpose names guessed from a platform (`XboxSystemStatement` is used 116 times in PS2 data, so the name is
     at best Xbox-flavoured) ❓.
- Older names from before the AI pass (`Game`, `Map`, `Entity`, `Trigger`, `Timer`, `Animation`, `DynamicMesh`
  and its fields) were assigned during the original reverse engineering and fit our data (§6).

## 5. PS2 status (high level)

Baseline smoke test with the stock tools: `vcs2json` decodes `.loc`; `.gam`, `.ls` and `.cin` segfault (one
small `.loc` returns rc 1). Our scans (§6, §9) suggest the schema applies to PS2 almost unchanged above the
bulk-data level. Agent `ps2-decode` is patching a copy for PS2 and documents that work in
[vicious-ps2.md](vicious-ps2.md); this review doesn't duplicate it.

## 6. Cross-check against our `notes/formats.md`

| Our section | vicious class / file | Verdict |
|---|---|---|
| DATA.PAK | `apps/pak/src/pakdump.c` | **Agrees** exactly |
| Container | `metadata.ts`, `file.ts`, `label.ts` | Agrees; **vicious adds** the file header and the class-per-chunk rule |
| Texture | `Texture` #10, `ImageTexture` #12 (`texture.ts`, `imageTexture.ts`) | **We got something wrong** (header labels); vicious adds field names |
| Sound | `Sound` #38, `SoundEffect` #42 (`sound.ts`) | Agrees; **vicious adds** names; the sample buffer is platform-specific |
| Collision | `V20_9`/`V20_10` in `staticMesh.ts`, `DynamicMeshEntry2` | **We know more** than vicious |
| Animation clip | `Animation` #46 (`animation.ts`, `apps/animation/src/anm2gltf.c`) | Agrees field for field; **vicious adds** track-flag meaning |
| Level objects / render mesh / materials | `StaticModelTemplate` #30, `StaticMesh` #20, `MaterialSetImpl` #37, `SingleTextureModelMaterial` #27, `StaticInstance` #103, `Model` #29 | Agrees on structure; render buffers are PS2-specific; **vicious adds** class ids, names and probably prop placement |
| Entity models (templates) | `ModelTemplate` #44, `DynamicMesh` #28, `Skin` #69 | Agrees; **vicious adds** names for everything we parse |
| Entities | `Entity` #140, `EntityTemplate` #142 (`entity.ts`) | **Agrees** exactly |
| Object reference | `AssetReference` (`asset.ts`), `Label`/`LabelID` | **Agrees** exactly; **vicious adds** that `type` is the class id |
| Bit stream | — | Not covered by vicious (it is a file-format project) |
| Game logic in data | ≈210 script classes | **vicious adds** the whole structure (§6.11, script-vm-plan.md §1) |

### 6.1 DATA.PAK ✅ agrees
`pakdump.c`: `u32 count`, `u32 version`, table at `0x800`, entries `char[40]` + `u32 sector` + `u32 size`.
This is exactly our layout.

### 6.2 Container ✅ agrees; vicious adds
- Magic, version 3, chunk header/footer and 4-byte alignment all agree. vicious also aligns before the footer, so
  `end` is always aligned.
- **File header** (`AssetFileHeader`, itself a chunk at `0x14`): `u32 0x20`, `u32 f_1`, `ID` (lo, hi) at
  `0x28`, `Label` at `0x30` (our display name at `0x34`), **`u32 class type` at `0x4C`**, `u32 f_2`.
  ✅ On all our files the type is `Map` (101) for 29/29 `.map`, `Game` (100) for the `.gam`, `LoadingScreen` (399)
  for 10/10 `.ls` and `Cinematic` (363) for 102/102 `.cin`. `.loc` files use the separate localisation layout.
- **One chunk per class level**: an object of class C opens with C's chunk, then its base class's chunk, and so on
  down to `Named` (label) and `Base`/`Expression` (no chunk). This explains our "a class's chunk usually starts
  with its base class's chunk".
- A **name** is `Label` = `u32 size (20)`, `char[20]`, `LabelID { u16 index, u16 unique }`. In our notes the
  trailing `u32` had no meaning. The same `{index, unique}` pair is how scripts address list members (script-vm-plan.md §1.3).

### 6.3 Texture ⚠ we got the header wrong
Our table reads the allocation from the `BEBEBEBE` guard. With the vicious names, the guard is the **footer of
the `Texture` base chunk**, and what follows is `ImageTexture`:

| Our offset | Our label | Actually (vicious) |
|---|---|---|
| guard − 8 … −1 | (not in our table) | `Texture`: `u8 format`, `u8 b2`, `u8 b3`, `u16 width`, `u16 height`, pad |
| 0 | guard | `Texture` chunk footer |
| 4 | has-mips flag | `ImageTexture.f_0x48` (bool, unnamed) |
| 5 | unknown | `f_0x49` (u8, unnamed) + alignment padding |
| 8 | "pointer (to small owned object)" | **chunk header** of `ImageTextureBuffer` (`BB BB BB BB`, end, version 1), not a pointer |
| 20 | "format: 1 = PSMT8" | **`enabled` flag** (pixel data present) |
| 21… | width, height, mips, pad | PS2-specific buffer layout (Xbox has `u32 levels` here) |

✅ Verified on all 6,359 textures in 33 maps (`texcheck.py`): the byte at 20 is 1 in every case; the `Texture`
base width/height 8 bytes before the guard equal our width/height in 6,359/6,359; the buffer chunk version is 1
in all of them. The **real format byte** (`Texture.format`) takes the values 0 (1,096), 1 (3,712), 2 (327) and 3
(1,224). Yet our 8-bit paletted decode works on all of them, so on PS2 that byte is not the pixel format
(on Xbox 1 = 4-bit, 3 = P8). ❓ What it means on PS2 (maybe alpha/blend mode) is open.

### 6.4 Sound ✅ header agrees; vicious adds names
`Sound` fields: `duration` (ms), `bitDepth`, `samplingRate`, `volume`, `f_0x50`, `disabled`, `f_0x54`. Our
example `bf 0c 00 00 | 10 | 44 ac | 1.0` is `duration = 3263 ms, bitDepth 16, 44100 Hz, volume 1.0` ✅ (2,794
headers found in the maps; then two bools and the float `10.0`). The Xbox `SoundBuffer` carries a WAV `fmt`
chunk. On PS2, 2,504 of 2,794 buffers hold `enabled`, `u32 32000` (sample rate), a float, `u32 n`, then exactly
`n` × 16-byte blocks before the chunk end (PS-ADPCM frames) 🟡. A direct reader built on this could replace the
frame-search heuristic in `gamedata.rs`/`sfx.py` (we decode 107/138 today). The other 290 differ (rate 44100 at
that offset) ❓.

### 6.5 Collision 🟡 we know more
Level collision is `StaticMesh` → `V20_9` (chunk, version 4) → `V20_10` (v1 chunk holding one `u32`), then an
`enabled` byte, and **the rest is an opaque buffer in vicious**. That matches our "base = v1 chunk with one
u32" and "u8 present". Our BVH/leaf/strip layout goes beyond vicious and is our own contribution. vicious
does give the **owners**:
- `DynamicMeshEntry2` (model "hit boxes"): `u32 node`, `Transform`, `Vector3`, `u16 type`, `u16`, plus an embedded
  `V20_9` when `version > 5 && type == 3`. Our `model.rs` reads the same thing and adds the shape meaning (0
  sphere, 1 capsule, 2 box, 3 mesh), which vicious lacks.

### 6.6 Animation clip ✅ agrees; vicious adds
`Animation` #46 (fields from version 3 on, = our "version-3 chunk"): `rate`, `length`, `frameCount`,
`jointFlagCount`, `looping`, `f_0x4d`, `f_0x4e`, `f_0x4f`, rotation/position/scale counts, `positionScale`,
`scaleScale`, `jointFlags[jointFlagCount]`, keys, three index tables, the per-frame table when `f_0x4f`
is set, then a byte buffer and an index table. **Same order and sizes as ours**; our `flags[0]` is `looping`.
`anm2gltf.c` adds playback semantics:
- per-joint flag 0 = no track; 8 = animated translation and rotation; 9 = translation only animated; 10 =
  rotation only animated; anything else = one constant key;
- the index tables map frame → key (de-duplicated keys);
- quaternions are /16384 with `w` negated;
- frame time step `rate / 500` s 🟡. This is their guess and doesn't obviously match our "frame_ms"; check
  against footage.

### 6.7 Level objects, render meshes, materials ✅ structure agrees; vicious adds
✅ (`lvlcheck.py`, all maps): **681 of 685** of our "level object v3" chunks are `StaticModelTemplate` (#30). Their
geometry is a reference to `StaticMesh` (#20, our v4 geometry; 683/685) and the material list is a reference to
`MaterialSetImpl` (#37; 684/685). Our material object ref type `0x1b` = 27 = `SingleTextureModelMaterial`.
- Our "base" of the geometry = `Mesh` fields `f_0x40`, `transparency`, `scale`, `inverseScale`; our "LODs (node
  tree)" = `StaticMesh.f_1` (list of `{u32,u32,u32, list of {i16,i16,Vector3,Vector3}}`).
- The render data (`V20_5`) is platform-specific. The Xbox has sections + vertex buffer + u16 indices; we have
  the PS2 strip blobs. Ours is correct for PS2; vicious doesn't help here.
- **Props at the origin**: vicious has `StaticInstance` (#103): object + `model` (`AssetFromType`, usually
  `Model` #29, which carries a `Transform` and a vertex buffer) 🟡. This is very likely the missing prop
  placement. Not yet checked against our props.
- Templates: our "v9 template" = `ModelTemplate` (#44). Our "[v2 name]" is its `DynamicGeomTemplate` base chunk
  (the name sits inside). ✅ In all 2,311 v9 templates across 33 maps, the next field is an `AssetReference` of
  class 28 = **`DynamicMesh`**: 2,207 inline (chunk version 6 in every case) and 104 by reference. Our "v6
  skinned/hierarchical model" is therefore `DynamicMesh` v6.
- `DynamicMesh` names what `model.rs` already parses: `viewBox` (our quantisation scale), `meshDeform` + 4 ×
  u16, `helperPoints` (= our 80-byte nodes: `i32 parentIndex`, `Transform`, `Label`), `transforms` (= our 48-byte
  "bones"), `hitBox` (116-byte entries from v4: `u32`, `Transform`, `Matrix3`, `Label`), `hitBox2` (above), a
  `Vector3` list + u32 buffer, optional collision, and `body` (Xbox: 32 `MeshSection`s with rigged/static ranges,
  rigged vertex + joint-weight buffers). Our PS2 body starts with the same "32 per-section counts".

### 6.8 Entities ✅ agrees exactly
`Entity` #140: `template_` ref, `u8`, ref (v>11), `u8 ×3`, `u32`, `u32` (v>6), `old u32` (7<v<13, absent at
v13), `i32` (v>10), `Transform` (position + 3×3 rotation, columns = our X/Y/Z axes), then lists, `EntityBody`,
and an optional `DynamicModel` (v>10). ✅ 2,758 entity (v13) chunks in all maps reference class 142
(`EntityTemplate`), 17 are null. Our "class definition v16" is `EntityTemplate` v16: `u8` (v>13), then the
`modelTemplate` ref (our `class_template()`), then behaviour lists. vicious adds the meaning of those lists
(script-vm-plan.md §1.1).

### 6.9 Object reference ✅ agrees exactly
`AssetReference`: `i16 type`, `i16 p2`, `ID` if `type ≥ 0`, `u32 p3` if `p2 > 0` (= our "flags ≠ 0 and bit 15
clear"), `Label`, `bool first`, then the inline object if `first`. **`type` is the class id** of the referenced
object (✅ 142, 28, 20, 37, 30, 79… in the checks above). Our "string (u32 len, chars, u32)" is the `Label`
with its `LabelID`.

### 6.10 Bit stream ❓ not covered
vicious only covers files. Its network classes are script statements/expressions (`NetworkClientStatement`
#424, `NetworkModeMatchExpression` #386, …), not the packet format.

### 6.11 Game logic in the data 🟡 confirmed and named
Our guess was right: the prefixes are **enumeration names**. `Enumeration` #171 holds `EnumerationEntry` #170
(name, `u32 id`, `enabled`), with 132,730 entry chunks over all files (class-id scan). These enumerations act as
the engine's symbol tables: `State`, `Msg*`, `Var*`, `Const*`, `Rel*`, `Cat*`, `Gs*`, `Time*`, `Anim*`,
`PCtrl*`, `SystemEvents`. `HL_*`/`LL_*` are **state machine** names. Details and counts:
[script-vm-plan.md §1](script-vm-plan.md).

## 7. What this means for our rewrite

### 7.1 README "next steps"
| Step | Effect of vicious |
|---|---|
| 1. Entity model format (v6) | **Mostly done already, and now named.** `model.rs::templates()` already reads `DynamicMesh` v6 (helper points, bind transforms, hit boxes, collision shapes, rigid body parts). The README row "❌ next" is out of date. What's left is skinning (rigged ranges/joint weights; Xbox layout known, PS2 layout ours to find) and binding `Animation` tracks to `helperPoints` by index (`anm2gltf.c` shows the mapping). |
| 2. Doors that open | The door is data: `Door_*` entities → `EntityTemplate` → behaviour slot (`V258`) → `StateMachine "Door"`. The direct path is the script VM ([script-vm-plan.md](script-vm-plan.md), milestone 1). A stop-gap without the VM: read the `AnimDoor` clips and the `VarDoor`/`ConstDoor` values. |
| 3. Map lights | **Now easy.** ✅ All 29 maps: 5,220 `PointStaticLight`, 190 `SpotStaticLight` (`coneAngle`), 119 `AreaStaticLight` (`width`, `height`), 90 `DirectionalStaticLight` (`azimuth`, `elevation`), 19 `SkyStaticLight`. Every one parses to its chunk end with the vicious layout: `StaticLight` = `f32 intensity`, `i32 color`, `i32`, `Transform`. Mansion_Story has 245 point, 15 area, 4 directional and 1 sky light; CreepyCarnival has 257 point, 21 spot, 2 sky, 1 area and 1 directional. |
| 4. Money, coins, safes, traps | All scripted (`Safe`, `Item_MoneySafe`, `Item_DoorTrap`, … state machines in Mansion_Story). Same route as doors. |
| 5. Reference views | Lights (3.) should move the `ref_compare` score most. |

Also new: prop placement via `StaticInstance`/`Model` (§6.7), `Trigger` volumes (`onEnter`/`onInside`/`onExit`
scripts + `BoundingVolume`), `Path`/`Point` (patrols, spawn points), and `Timer`.

### 7.2 Licence and clean-room position
- vicious is MIT per its package manifests, but the repo has no LICENSE text, so attribute it by name and URL
  plus "MIT, © Alexandre Lavoie".
- **Format knowledge** (class ids, names, field order) is fact, and documenting it in our own words in `notes/`
  with a citation, as here, is fine.
- **Our code stays ours.** We write our own Rust readers from these notes. We don't copy or link vicious C/TS
  into `game/` or `tools/`. If we ever copy a file, it carries the MIT notice at the top (house rule in the
  brief).
- Add a credits line to `README.md` when the first vicious-informed feature lands (not done in this review).
- vicious was itself built from the Xbox build. Nothing from our Ghidra decompile goes into notes, and this
  review uses only data-side evidence.

## 8. What it means for the friend (Ghidra on the PS2 ELF)
- **Class ids → names → PS2 loaders**: agent `class-map` matches the 518 class ids to PS2 loader functions:
  [class-map.md](class-map.md) and `notes/class-map.csv` (id, name, vicious file, Xbox offset, PS2 loader,
  versions seen, confidence). Use it to rename loader functions and their vtables in your Ghidra project.
- **Field order per class**, including version gates, is in each `packages/core/src/schema/types/<class>.ts`. On
  PS2 a loader reads its fields in that order (base-class loader first, then own fields), so field names can go
  straight onto the struct the loader fills.
- **Data facts the code will show** and which we can't settle from data alone (most useful to us if you find
  them):
  | Question | Where to look |
  |---|---|
  | `BinaryOperationExpression.operation` 0–16, `UnaryOperationExpression.operation` 0–19, `AssignValueStatement.operation` 0–4 | the evaluator `switch` for classes 111 / 110 / 120 |
  | `ExecutionContextValueExpression` `source` 0/1/2 | its evaluate function |
  | message dispatch: who receives `V192` messages, and the order of state `Enter`/`Exit`/`Update`/`Timer` | the state-machine runtime |
  | `Texture.format` 0–3 on PS2 | texture upload path |
- **`LabelID {index, unique}`**: scripts address a state or local function by its index in the owning list
  plus a "unique" tag (script-vm-plan.md §1.3). Expect `index` to be used and `unique` to be checked or ignored.
- Names that came from the data (enumeration and state machine labels such as `MsgDoor`, `VarDoor`, `HL_SpyBase`)
  are reliable. Names that came from vicious' AI pass are guesses (§4.4).

## 9. Verification log
All scripts are in `work/review/`, run from the project root with `python3 -I` over `work/x/` (our extraction:
29 maps, 1 `.gam`, 102 `.cin`, 10 `.ls`, 528 `.loc`).

| Claim | Command | Result |
|---|---|---|
| 518 classes, ids 1–518, 85 `__todo`, 939/1,359 unnamed fields | `parse_schema.py … schema.json`, `areas.py` | as stated |
| AI commit = names only | `parse_schema.py` on `git archive d621a56^` → `before.json`; diff by id | 132 class + 185 field renames, 0 layout changes |
| `apps/edit` error | reconfigure in `work/review/vicious/apps/edit` | missing `glfw3` (pkg-config) and OpenGL dev files |
| File header class type at `0x4C` | inline script over `work/x/**` | 29/29 map = 101, gam = 100, 10/10 ls = 399, 102/102 cin = 363 |
| Texture header | `texcheck.py` | 6,359/6,359: byte 20 = 1, base w/h match, buffer chunk v1 |
| Sound header / PS2 buffer | inline scripts (`sound` probes) | 2,794 headers; 2,504 buffers = `32000`, f32, `n`, `n`×16 B |
| Level objects | `lvlcheck.py` | 681/685 → #30, 683 → #20, 684 → #37 |
| Templates | `tplcheck.py` | 2,311 v9 → mesh ref class 28; 2,207 inline, all v6 |
| Entities | `entcheck.py` | 2,758 v13 → class 142; 17 null |
| Lights | `lightcheck.py` | 5,638 lights; `StaticLight` own bytes = 60 in all; subclass sizes 0/4/8/4/12 as schema |
| Script node counts | `scriptcount.py` | 819,162 statement/expression nodes; 99.3% classified by id |
| Reference targets in scripts | `scriptrefs.py` | see plan §2 |
| Containment (who holds what) | `graph.py`, `doorsm.py`, `names.py` | see plan §1 |

`classscan.py` (a whole-file histogram of class ids by introducer) is **noisy**: any small `u32` before a chunk
looks like a class id. Its numbers are only quoted above where a second check backs them (lights, enumeration
entries).
