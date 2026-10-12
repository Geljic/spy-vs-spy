# Vicious Engine class map (PS2 SLES_530.78)

Status legend: ✅ verified on real PS2 data or the ELF · 🟡 partial · ❓ unknown

This page explains `notes/class-map.csv`. The CSV lists every class id in the vicious schema (518) with
the function in the PS2 ELF that deserialises that class, the versions of that class found in our PS2
data, a confidence and the evidence. `tools/ghidra/ApplyClassMap.java` reads the CSV and names those
functions in a Ghidra project in one go.

Lanes: this is data-side work. It gives names, ids, versions and field lists. It does not describe what
each loader does internally; the vicious schema file named in each row holds the field list.

## Coverage ✅

| | Classes |
|---|---|
| Class ids in the vicious schema (Xbox) | 518 |
| Class ids the PS2 ELF registers | 469 (all of them are vicious ids too; none are PS2-only) |
| Vicious ids the PS2 ELF never registers (Xbox-only or unused) | 49 |
| PS2 loader found | 468 |
| … confidence **high** (version gates and read order match the schema) | 276 |
| … confidence **medium** (anchored by the id table, read order not confirmed) | 187 |
| … confidence **low** (loader inferred, not anchored) | 5 |
| Registered, but no loader found (`MaterialSet`, id 35) | 1 |
| Classes found in the PS2 data as the object's own class | 349 (376 including base-class chunks) |

ApplyClassMap applies the 463 high + medium rows.

## How the engine registers classes ✅

- Every class has a **class-info record** in one global table of 0x30-byte slots indexed by class id.
  The registration helper (at `0x13a6e8`) fills the slot on first use: class id, parent class record,
  a **create function** (allocates and constructs an instance), a **folder name** and a **file
  extension** (for example `Entities`/`ent`, `EntityTemplates`/`et`, `StateMachines`/`sm`), and flags.
  A class that passes no folder inherits its parent's.
- Each class has a small lazily-initialised getter that registers it and returns its record. The
  getter for class N is inlined at every place that needs it; we found **469 distinct ids** this way.
- Objects are C++ objects with a GCC 2.x vtable at offset 4 (8-byte entries: two 16-bit `this`
  adjustments and a function pointer). Slot `+0x0c` returns the class record; slot **`+0x2c` is the
  class's deserialise ("load from stream") method**. Every class with a vtable has its own load method;
  none of them reuse the parent's.
- Each load method opens a chunk with the chunk-begin routine (`0x13d1f0`), passing **the newest
  version this build writes**, then calls its parent's load method, then reads its own fields, with
  `version > N` style checks for fields added over time. This matches the container layout in
  `formats.md#container` and the vicious schema's versioned fields.
- The object-reference reader (`0x13e0a8`) indexes the same class table with the reference's `i16 type`;
  the typed-object reader (`0x13d520`) reads an `i32` class id and calls the create function from the
  table. So **the type in an object reference is the class id** (verified on data below).
- Class ids agree with vicious: the parent ids the PS2 registers match vicious' `base` class for
  461 of 469 ids, and the folder/extension (inherited from the parent where a class gives none)
  matches vicious for 460 of 469. The rest are spelling differences (`SurfaceSets`, `StateMachines`,
  `External Variables`, `Camera Controllers`, `LensFlares` on PS2; `Cinematic`/`Cinematics` swapped
  between `CinematicContent` and `Cinematic`) and the two material-set classes that PS2 files under
  `MaterialSets`/`.ms` where vicious inherits `SurfaceSet`/`.ss`.

## How the map was built

1. **Schema side.** Class ids, names, folders and extensions come from vicious' generated `util.c`;
   the per-class field order and version conditions come from its generated `decoder.c` (which is
   generated from `packages/core/src/schema/types/*.ts`); file names and Xbox offsets come from the `.ts`
   files.
2. **Registration table.** Every call to the registration helper in the bulk decompile was collected:
   id → parent, create function, folder, extension (469 ids, consistent across all 5,434 call sites).
3. **Id → vtable → loader**, two independent ways that agree on 399 ids:
   - follow each create function to its constructor and take the vtable it stores;
   - scan the ELF's data segment for vtables whose class-info slot leads (through a getter) to the
     global holding class N's record.
   The loader is the function in vtable slot `+0x2c`. This gives 454 loaders. 14 abstract classes have
   no vtable of their own; their loader was taken from the base-class call that all their subclasses'
   loaders make first (unanimous with 2 or more votes for 9 of them, a single vote for 4, split for 1).
4. **Loader check.** All 468 loaders were decompiled headless (on a copy of the Ghidra project; 148 of
   them were not functions yet because only vtables point at them). For each one we compared, against
   the vicious field list:
   - the set of version boundaries it tests (ignoring the common "version 0 = no data" check);
   - whether the first call is the parent class's loader;
   - the order of reads: 1/2/4/12-byte reads, object references, labels, sub-class loaders and nine
     struct-reader helpers learned from the data (table below).
   **high** = boundaries equal, base call consistent, and at least 75 % of the schema's fields appear
   in the same order (or an exact match for small classes). **medium** = loader is anchored by the
   registration table and vtable, but the read order could not be confirmed (usually because the PS2
   loader inlines a list or calls a helper we did not map). **low** = loader inferred from subclasses
   without a unanimous vote.
5. **Data side.** A scanner (scratch script, not in `tools/`) walked all 710 container files in
   `work/x/` (568 `.loc`, 102 `.cin`, 29 `.map`, 10 `.ls`, 1 `.gam`). A chunk is attributed to class N
   when it directly follows an `i32` N (typed object) or an object reference of type N whose inline
   flag is set; then the nested base-class chunks are attributed to the parent classes. The CSV's
   `ps2_versions_seen` gives `version:count` for chunks where the class is the object's own class, and
   in brackets the versions seen when it appears as the base part of a subclass.

Struct-reader helpers learned while aligning (not classes, so not in the CSV):

| PS2 address | vicious struct it reads | aligned in loaders |
|---|---|---|
| `0x157628`, `0x1bc4e8` | `AssetFromTypeWrap` | 114 + 2 |
| `0x13d520` | `AssetFromType` (i32 class id + object) | 52 of 56 |
| `0x13ada8`, `0x13a9d0` | `AssetFromTypeSizedList` | 40 of 42, 2 |
| `0x156db8` | `FN_0x22520` (dialog element reference) | 21 |
| `0x152d20` | `FN_0x21f40` | 16 of 18 |
| `0x172828` | `ExpressionList` | 11 |
| `0x152a10` | `FN_0x21dd0` | 2 |

## Verification ✅

- **Object reference type = class id.** Of 59,691 inline object references in the data, 59,613
  (99.9 %) open a chunk whose version is exactly the newest version the PS2 loader for that type
  writes. Material references carry type 27 = `SingleTextureModelMaterial` (5,223 of its 5,270
  chunks are v5, its loader's newest).
- **Typed objects.** 1,670,734 chunks attributed via a preceding `i32` class id; 1,640,188 (98.2 %)
  are at that class's newest version (the rest are older versions the loaders still accept).
- **Our earlier anchors land on the right classes:** entity loader `0x15a848` = `Entity` (140, v13,
  2,714 chunks), level geometry `0x236e38` = `StaticMesh` (20, v4), skinned model `0x209660` =
  `DynamicMesh` (28, v6), animation clip `0x1fd368` = `Animation` (46, v3), base loader `0x1410f0` =
  `Object` (3). (Collision and render data are struct readers inside those classes, not classes.)
- **ApplyClassMap, headless on a fresh copy of the project** (`work/class-map/gp2`, `nice -n 19`):
  461 functions renamed, 148 of them created first, 2 user names kept (`LoadModelBaseChunkV3` = Mesh,
  `LoadModelChunkV6` = DynamicMesh), 463 plate comments, 0 failures. A second run renamed 0 and added
  0 comments (safe to re-run).

## How the friend should use it

1. Put `notes/class-map.csv` next to the project (or pass its path) and run
   `tools/ghidra/ApplyClassMap.java` from the Script Manager (category *SpyVsSpy*). Headless:
   `analyzeHeadless <dir> <project> -process SLES_530.78 -noanalysis -scriptPath tools/ghidra -postScript ApplyClassMap.java notes/class-map.csv`
   (add `all` as a second argument to also apply the 5 low rows).
2. Each named loader gets a plate comment with the class id, the versions present on the disc and
   the vicious schema file. Open that `.ts` file next to the decompile: the field order is the read
   order, and `ctx.version() > N` is the loader's version check.
3. From a named loader, the parent's loader is the first call, and the vtable that points at it
   (`evidence` column) gives the class's other virtual methods; the create function and constructor
   are reachable from the registration table at `0x41c920` + id × 0x30 (+0x10 = create function).
4. Classes at **medium** are still the right function for that id; only the field-by-field check is
   missing. The rows whose evidence says "version gates differ" (50 classes, e.g. `Game` v22,
   `UIElement`, `SpawnEntityStatement`, `Sky`) are where the PS2 build reads different fields from the
   Xbox schema; that is where to look if vicious misreads PS2 data.

## Caveats

- 🟡 The data histogram misses objects that are not introduced by a class id in the stream. The
  top-level objects of each file (for example the `Game` object in the `.gam`, whose id is written
  in the file's table at the start) are not counted, so `Game` shows no versions.
- 🟡 Counts in brackets (as the base part of subclasses) assume the first nested chunk inside a class's
  chunk belongs to its parent. That holds when the loader calls its parent first; for a few classes
  (`Expression`, `Texture`, `Material`, `V184`) it over-counts and lists versions above the loader's
  newest. The direct counts do not have this problem.
- 🟡 The high/medium split depends on how well the decompiled read calls can be tokenised; medium often
  just means the loader reads a list inline. It does not mean the loader is wrong.
- ❓ `MaterialSet` (35) is registered but has no vtable and none of its subclasses calls a shared
  base loader, so it has no loader. Eight PS2 parents differ from vicious: `MaterialSet`, `V36`,
  `MaterialSetImpl`, `V41` (PS2 puts the material-set family under `Object`/`MaterialSet`, vicious
  under `SurfaceSet`), `Statement`/`ValueExpression` (PS2 parent `Expression` 106; vicious names
  `ExpressionWrap` instead), `V291`, `V378`.
- Low rows: `V50`, `V54`, `Expression` (106), `V369`, `V403`: one-vote or split inference.
- 49 vicious ids (`V6`–`V8`, `NullMaterial`, `V98`, `V99`, … `V517`) are never registered on PS2.
- The scratch scripts and decompiles live in `work/class-map/` (gitignored); nothing decompiled is
  in this note or the CSV.
