# spy-vs-spy

A clean-room PC rewrite of **Spy vs. Spy** (PS2, 2005). It reads game data from
your own disc dump at runtime. This repo contains no game files, no extracted
assets and no decompiled code.

## Where we're at (2026-10-05)

**Focus:** two maps only, finished properly before any others: **Mansion_Story** (matches Simon's reference
footage) and **CreepyCarnival**.

### What works / what's missing

| Area | Status |
|------|--------|
| DATA.PAK archive (list/extract, straight from ISO/MDF) | ✅ `tools/pak.py`, `game/src/gamedata.rs` |
| Asset container (chunked object stream) | ✅ structure; per-class layouts decoded as needed |
| Textures (8-bit paletted, swizzled) | ✅ named by their owning chunk (fixes the earlier off-by-one names) |
| Sound effects (PS-ADPCM) | ✅ 107/138 decode, used in classic mode |
| Level collision geometry | ✅ all maps, used for walking |
| **Level render meshes + materials** | ✅ textured levels in arena mode; `tools/levelmesh.py` exports OBJ+MTL |
| Sky domes, light shafts, cut-out/blended alpha | ✅ sky unlit; soft-alpha textures blended (light shafts additive) |
| **Placed entities** (player/AI start, doors, safes, lasers, ...) | ✅ decoded (v13 chunk: class ref + position + axes); spies start at the real start points |
| Animation clips | 🟡 layout decoded; needs skeletons |
| Entity models (doors, safes, spy character) | ❌ next: v6 skinned/hierarchical model format (`FUN_00209660`) |
| Doors opening, safes/shop, coins, money HUD | ❌ (seen in the reference footage) |
| Lighting | 🟡 approximated; map light objects (`Directional`, `D_Fill`, ...) not decoded yet |
| Classic mode (rooms, search, traps, fights, AI, escape) | ✅ playable prototype |
| Arena mode (walk the real levels) | ✅ prototype; spies are placeholder figures |
| Linux + Windows builds | ✅ (Windows exe cross-built, tested under Wine) |
| Online multiplayer | not started (engine's network bit-stream format found) |

### Known problems right now

- **Mansion_Story arena test fails**: walking from the real start points, the test spies reach areas with
  no level collision and fall out of the world. Those gaps are covered in the original by entities we don't
  simulate yet (doors, bridges, platforms). Closed doors are approximated by invisible walls at `Door_*`
  entities; that wasn't enough. A "don't step into the void" rule was proposed and not applied.
- Some corridor ceilings / props are still missing in places; a few materials are untextured.
- Lighting is brighter and flatter than the PS2 (reference score 78.3 for the pillared hall).

### How we check things

- `game/tests/run_all.sh` – gameplay scripts (scripted inputs → expected end state) + walk test on the focus maps.
- `game/tests/ref_compare.py` – renders a fixed camera view and puts it next to a frame of Simon's PS2
  recording (`tests/ref/views.txt`), with a rough similarity score to track over time.
- Debug: `SVS_CAM="x,y,z,tx,ty,tz"` pins the camera; `SVS_SHOT_DIR` sets where screenshots go.

### Next steps (in order)

1. Decode the entity model format (v6) → real doors, safes and the spy character.
2. Doors that open (and block until opened), then fix the walk test.
3. Map lights → lighting closer to the reference.
4. Gameplay from the footage: money, coins, safe shop, traps.
5. More reference views from the footage, tracked with `ref_compare.py`.

Details of every format: [notes/formats.md](notes/formats.md)

## Playing

```sh
cd game
cargo run --release                         # classic mode, you (White) vs AI (Black); F2 = 2 players
SVS_MAP=Mansion_Story cargo run --release   # walk a real level (focus maps: Mansion_Story, CreepyCarnival)
SVS_DEMO=1 cargo run --release              # watch AI vs AI
tests/run_all.sh                            # all test scripts (gameplay + every arena)
```
The game looks for your disc in `$SVS_DISC`, next to the executable, or `~/Games/Downloads/Spy vs. Spy/`.
Windows: `cargo build --release --target x86_64-pc-windows-gnu`, then set `SVS_DISC` to your `.iso`/`.mdf`.

## Tools usage

Needs Python 3 and Pillow. Extracted data goes in `work/`, which is gitignored.

```sh
python3 tools/pak.py list    "/path/to/Spy vs. Spy.mdf"
python3 tools/pak.py extract "/path/to/Spy vs. Spy.mdf" work/x [Maps/]
python3 tools/tex.py work/x/Maps/5C35DB16C6D45BCC.map work/tex/CreepyCarnival
```

## Rules

Bring your own copy (PAL SLES_530.78). See `docs/ai-game-modding-guides/06-rules-legal-and-publishing.md`
in the monorepo.
