# Handover: spy-vs-spy

Updated 2026-10-19 (evening) after the menus run end-to-end in the friend's engine: PR #1 on
LuckyDucko/SpyVsSpy, 16+ commits on branch `ui-menus`. Direction: **we support the friend's C++ engine,
UI first** (his brief: "stick to UI for now").

## Start here (paste as the first prompt of the next session)
> Continue spy-vs-spy (projects/spy-vs-spy). Read CLAUDE.md, HANDOVER.md and notes/ui.md first. In
> `work/friend-SpyVsSpy` run `git fetch` and `gh pr view 1 --repo LuckyDucko/SpyVsSpy --comments`: merged?
> did it build on his Mac? Then `ci/build_test.sh` (expect ALL PASS on Linux). Work down "Next" from 1.
> Mute any test run per process (never pactl). Use subagents for independent pieces (memory:
> feedback-orchestration).

## State (verified this session, Linux)
- **Friend's repo** github.com/LuckyDucko/SpyVsSpy (private, Simon has push), clone `work/friend-SpyVsSpy`,
  branch `ui-menus` pushed, PR #1 open. `ci/build_test.sh` **ALL PASS (15) on Linux** (macOS paths kept,
  not re-run on a Mac).
- **What works** (all running the game's own scripts from the disc):
  - `src/ui/menu.hpp`: Dialog/grid/text/sprite model, layout rules from the ELF and checked against the
    reference video (y-up UI, safe area 48/36/544x408, grid frames, fills, fonts, rotation, backgrounds).
  - `src/ui/script.hpp`: script VM + runtime: expressions/statements, functions, state machines with timers,
    messages, dialog stack with layers/callbacks/results, focus + grid navigation, selection lists,
    strings, entity variables, game state, virtual memory card, movies (Host hook), loading screens, boot.
  - `poc/12_menu`: `run.sh` = static test + **flow** test: boot (lock-file + card check, intro movies
    Legal/GlobalStar/Mad/VCS/Gamespy/Intro) → title → Start → Spy Files (load SIMON) → main menu → Single
    Player → Story → Select Spy → Select Map → Mgr_Scene/HL_SpyMainMenu → IntroStoryBlack → ChangeMap
    `Game.maps[26]` = **Mansion_Story 4DC0D0DAD36EB8EF** → Mansion loading screen (book page, 4 challenges).
    Plus empty card ("No Spy Files found") and new player (create "TYSON" via keyboard → save → main menu).
  - `poc/13_menu_vk` (subagent): Vulkan renderer of the same draw list (matches the CPU raster), `visible`
    window with keys; `play.sh` chains its map change into poc/11 (never run here: no windows).
  - Linux builds for every PoC (`tools/linux_deps.sh`, `glslc_mini.c`; subagent).
- **Reference video** (Simon downloaded Tyson's "Spy Vs Spy (PS2) Full Game.webm" to ~/Downloads): menu
  timestamps in notes/ui.md §8; frames in `work/friend-SpyVsSpy/work/vidref/menu/`. Our screens match it
  closely (pairs in `work/friend-SpyVsSpy/work/poc12/*pair*.png`).
- poc/13 also draws the **3D MainMenu room** behind the menus (subagent, reviewed + committed `eb544c6`):
  cameras follow the scripts (`Host::camera` → map `Cam_*` points), framing matches the video. Missing: spy and
  prop models, faithful lighting, camera easing; title folder (Cam_TitleShot) renders upside down.
- Also in: SetUISprite, `ui::DiskCardHost` (Spy Files in a file), camera hook, boot sequence.
- **Our repo** github.com/Geljic/spy-vs-spy (public): uncommitted HANDOVER.md, `.gitignore` (+HANDOVER.md),
  `notes/ui.md`, `tools/script_print.py`. Not pushed (Simon didn't ask).
- Ghidra (`work/ghidra_proj`): UI/font/dialog/script functions named and saved; V138 load + EC value created.

## Next (priority order)
1. **Menu room fidelity**: draw the menu spy + props (DynamicMesh v6, MenuSpy1–4, Trophy_*), fix the title
   folder orientation, lighting closer to the video. Check the subagent's two engine findings in poc/09/11:
   level mesh needs its StaticInstance transform; `1 − v` texture flip looks wrong.
2. **Hand-off to gameplay**: after ChangeMap, poc/11 should start from the loading screen (and play
   `IntroStoryBlack` via poc/05 first). Agree with the friend how the engine hosts maps (one app vs PoCs).
3. Wire `ui::DiskCardHost` into poc/13's host so Spy Files persist in the interactive build.
4. Small gaps vs the video (notes/ui.md §7): challenge text inset, name-field cursor, keyboard key highlight.
5. Read the 🟡 rows in code (notes/ui.md §6.1), and keep the static/flow tests green.

## Unverified / known broken
- Nothing run on macOS. `visible` modes and `play.sh` never run (no windows on this PC).
- 🟡 rules are inferred from data + video, not read in code (listed in notes/ui.md).
- Many script classes still skipped (cameras V163, animations V193, entity relations); counted in
  `Runtime::skipped` and harmless for the menus.

## How to verify
```sh
cd projects/spy-vs-spy/work/friend-SpyVsSpy
ci/build_test.sh                          # ALL PASS (15) on Linux
poc/12_menu/run.sh                        # PASS PASS PASS (static, flow, create Spy File)
P12_TRACE=1 poc/12_menu/run.sh flow work/poc12   # the dialog/state/message trace
```

## Gotchas
- **The UI is y-up.** Numpad 7 8 9 = bottom; offsets go up; margins left/top/right/bottom = vicious
  f_0x50/52/54/56. Vicious field names are Xbox offsets (PS2 object = +4 for UI classes).
- Simon's `.mdf` is a Russian fan translation; use the DNAS-bypass ISO (= the friend's disc, sha256 a5d5b1d7…).
- Element flags: 0x2 shown (V148), 0x1 enabled (V147), 0x400 menu item; dialog 0x400 menu screen,
  0x800 pop-up, 0x1000 whole screen.
- His repo: product C/C++, `helpers/` read-only (a copy of our repo), agents read docs/NOTES.md +
  docs/HANDOFF.md; status lines tagged `[OS, agent]`. Commit as Simon with the noreply email.
- When editing scripts with sed/python, run `bash -n` and gate the commit on it (play.sh slipped once).
- Concurrent agents: commit only your own paths; the other agent's files stay unstaged until reviewed.

## Key files
- `notes/ui.md` (= his docs/UI.md): every fact with ELF address, the flow, the video timestamps.
- `tools/script_print.py`: pseudo-code dumps of dialogs/functions/state machines from vcs2json JSON.
- `work/friend-SpyVsSpy/src/ui/{vicious_file,menu,script}.hpp`, `poc/12_menu`, `poc/13_menu_vk`.
- `notes/vicious-ps2.md`, `tools/vicious-ps2/`: the PS2 decoder patches (vendored in his `third_party/vicious`).

## History
- 2026-10-19: reviewed both repos; PR #1: vicious C decoder, menus PoC, script VM + runtime (full menu →
  mission flow, boot, Spy File creation), Vulkan viewer + Linux builds (subagents), video-checked layout.
- 2026-10-12: vicious matches PS2; 710/710 files decode; class map; script-VM plan; repo public
  ([work log](notes/work-log-2026-10-12.md)).
- 2026-10-05..08: DATA.PAK extractor, format tools, Rust arena/classic prototype, Ghidra + EE plugin.
