# spy-vs-spy

> Clean-room rewrite of Spy vs. Spy (PS2, 2005, Vicious Cycle) that reads the player's own game data at runtime, with online MP via our own netcode.

## Scope
- Files for this project live in `projects/spy-vs-spy/`
- Do not create files outside this directory

## Context
- **Purpose:** Native PC version of Spy vs. Spy (PAL SLES_530.78) built from documented file formats, not copied code
- **Owner:** Simon
- **Status:** Active — phase 1: reverse-engineer DATA.PAK and asset formats
- **Game data (never copy into repo):** `/home/simon/Games/Downloads/Spy vs. Spy/` — `Spy vs. Spy.mdf` is Simon's own dump (same build as the bypass ISO)
- **Reference guides:** `docs/ai-game-modding-guides/` (esp. 03 and 06)

## Project-Specific Rules
- Whitelist `.gitignore`: nothing from the game, nothing extracted, no Ghidra DBs, no decompiled code in the repo
- Extracted / scratch data goes in `work/` (gitignored)
- Findings are written as prose/spec docs in `notes/` — that's the shareable output
- Tools read the player's own ISO/MDF; they never ship data

## Phases
1. DATA.PAK container → file list + extractor (`tools/`)
2. Identify inner formats (textures, meshes, maps, audio, scripts)
3. Viewer: load and render one map
4. Gameplay: spy movement, traps, rules
5. Netcode: 2–4 player online

## Ghidra (SLES_530.78)
- Use Claude-driven Ghidra (the `ghidra` MCP server) for questions about the ELF: far faster than
  one-off scripts, and names persist. Process, setup and rules: `docs/reverse-engineering-workflow.md`
  (monorepo).
- Project: `work/ghidra_proj/svs.gpr` (gitignored). Start with `tools/ghidra-server.sh` (`stop` / `status`).
- The ELF needs the PS2 (R5900) processor extension, which is in this project's own Ghidra copy
  (`work/tools/ghidra_12.1.4_PUBLIC`); the wrapper uses it. Older bulk output: `work/decomp/all.c`, `funcs.pkl`.
- Good first targets: the DATA.PAK reader and the inner format loaders (find them from file-name
  strings and their cross-references), which beats guessing formats from the data alone.
- Clean room: findings go in `notes/` in our own words; no decompiled code, no Ghidra DBs.

## Key Files
- `tools/` — Python extractors/format readers; `tools/ghidra-server.sh` starts the Ghidra MCP server
- `notes/` — format specs and findings log
- `src/` — engine (later; Rust/Bevy or Godot TBD)
- `work/` — gitignored scratch/extracted data
