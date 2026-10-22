# Handover: spy-vs-spy

Updated 2026-10-22 (evening) after wave "r": core restructure for Tyson's agent, sky/void check, app loop
part 1 (WIP). All merged on `mission-2c` @ ba950d9, pushed to LuckyDucko/SpyVsSpy (ci ALL PASS 16/16, m2 PASS 86 ok).

## Start here (paste as the first prompt of the next session)
> Continue spy-vs-spy (projects/spy-vs-spy). Read CLAUDE.md, HANDOVER.md and notes/work-log-2026-10-22.md
> ("Wave r"). In `work/friend-SpyVsSpy`: `git fetch` — has Tyson pushed to `mission-1`, merged `mission-2c`,
> or claimed anything in `MandateFromHeaven.md` §4.5? Run `ci/build_test.sh` (ALL PASS) + `poc/11_gameplay/run.sh m2`
> (PASS), then work down "Next" from 1.

## Team / split
- Tyson (LuckyDucko, repo owner, good friend; plain messages fine) splits by mission; **Simon owns Mission 2**
  (Kooky Carnival) + the shared HUD, and runs the core restructure (Mission 1 must keep working).
- **Claims**: `MandateFromHeaven.md` §4.5, `[claimed by X @ date]`, push before starting, first push wins.
  Ours claimed 2026-10-22: engine split, app loop, file split, agent docs, M2 map polish.
- Tyson's priority: **seamless map over mini-games; never sky/void below the spy.**
- Rules: mission-agnostic `engine/`, per-mission `missions/`, `helpers/` read-only, files ≤ ~500 lines,
  NOTES §9 line `YYYY-MM-DD [OS, agent] …` (1–2 lines), commit as Simon (noreply email).

## State (verified 2026-10-22, Linux, RTX 3070)
- `mission-2c` = Tyson's `mission-1` @ e4d006f + our m2c wave + wave r. Tyson has pushed nothing since.
- **Restructure done**: `poc/11_gameplay/main.cpp` 2434 → ~190 lines; state in `p11::App`/`Gfx`
  (`engine/app.hpp`), engine in `engine/*.hpp`, campaign layer in `campaign/` (minigame, flow, modelprop,
  test_harness, mission_start, app_loop). Big src/ + missions/ files split behind umbrella headers (always
  include the umbrella). Every source file ≤ 500 lines. Frames byte-identical to before the split.
- **Agent docs**: `AGENTS.md` (start page), folder READMEs, NOTES 555 → ~275 lines (history in `docs/archive/`),
  Mandate = per-mission Works / Doesn't work lists + claims. Start-up reading ≈ 9k tokens (was ~70k).
- **Quiet tests**: `PASS <mode>: N ok (Ns)`; full logs in `work/logs/`; `VERBOSE=1` for the old output.
- **Sky/void check** in `run.sh m2`: route sampled every 6th step (new void spot = FAIL, 4 known spots listed
  in `VoidLog::known`, carnival2_test.hpp) + whole-map survey (`M2_VOID_MAX` = 592 allowed cells, run.sh).
  Coaster rock ridge now uses its own render mesh as collision (void cells 4226 → 592, west park fixed).
- **App loop part 1 (WIP)**: `run.sh apploop` = menus → Mansion → victory → menus → KC_Story unlocked and
  started, then **segfaults loading the 2nd mission in one process**. `play.sh` still uses the old
  two-program path (poc/13 menus → `run.sh launch`), unchanged and working.
- Video: `dist/spy-vs-spy-m2-2026-10-22.mp4` (scripted m2 run at 3×, title/end cards), made by
  `tools/m2-video.sh`.

## Next (priority order)
1. **App loop crash**: 2nd mission in one process segfaults on load. Repro + suspects (function statics
   in carnival2_play_world.hpp:81, carnival2_coll.hpp:121, src/ui/vicious_file.hpp:89, level_lights
   `injected`, render_shot `acquired`; or freed GPU handles after `VkCtx::free_tracked`):
   `P11_SAVE=work/poc11/apploop_save P11_ENDTEST=continue P11_SHOTS=1.5,4.5,9,10.5 work/poc11/bin apptest work/poc11/apploop`
   under gdb. Then add `apploop` to ci, `play.sh` → thin launcher (`poc/13_menu_vk/run.sh pak` +
   `run.sh app`), menus back at main menu (not title), docs (READMEs, AGENTS.md commands, Mandate line).
   Design notes: top of `campaign/app_loop.hpp`.
2. **Remaining void spots** (Mandate M2 "Doesn't work"): fun-house walkway / upper mouth bridge / laser tube
   collision ~1.1 m above the drawn floor (the 4 known route spots); cage corridor (18..28, −44..−34);
   lift tops (stand-in boxes); maze goal-room crawl space (survey only); thin terrain-edge strips;
   tilt-a-whirl pit has no collision. Lower `M2_VOID_MAX` and empty `VoidLog::known` as they're fixed.
3. **Movers with the map's own models** (lifts `CC_Platform_01/02`, door leaves, cyan marker boxes) + one door
   system (flow's `State::open_doors` boxes vs `coll_set_door`); white strip on the big-top ring floor.
4. Mission 2 leftovers: helmet placement, entity models not drawn (booth/bell `ca_strongbell`, ducks, turret
   gun, `mnky_wheel`, clown), enemy projectiles, boss suicide monkeys, Kooky Cook + pizza ride, maze trapdoors.
5. Engine leaks: Mansion door code in `engine/level_mesh.hpp` / `render_actors.hpp`, Mansion checks in
   `campaign/test_harness.hpp`, `sim.hpp` includes `campaign/flow.hpp` → Mission hooks.
6. Video playback (disc `.mov`s) + the map's own trigger scripts; 16:9 after Missions 1 + 2 match.

## Unverified / known broken
- Nothing played by hand in a window (all headless). `run.sh app` (visible loop) compiles, never opened.
- App loop: 2nd mission crash (above); `run_loop` prints PASS after each mission.
- Look's brightness scale and shader-flag meanings are guesses matched to video frames.

## How to verify
```sh
cd projects/spy-vs-spy/work/friend-SpyVsSpy
ci/build_test.sh                    # ALL PASS (16)
poc/11_gameplay/run.sh m2           # PASS m2: 86 ok (incl. sky/void check); VERBOSE=1 for ok lines
poc/11_gameplay/run.sh test         # Mansion + launch + hud + weapons + actors
poc/11_gameplay/run.sh apploop      # WIP: fails at the 2nd mission
../../tools/m2-video.sh             # video of the m2 run → ../../dist/
```

## Gotchas
- No audio in the engine; testing is silent. Simon allows visible windows, never audio.
- `work/poc11/bin` is per checkout: don't run two `run.sh` in one checkout at once (it rebuilds).
- Agent worktrees `work/wt/r-{engine,split,docs}` (merged); data symlinked (ISO, deps, work/{verify,vidref,venv}).
  `/deps` is in `.git/info/exclude` (the symlink isn't matched by `deps/`). Never `git add -A` there.
- Part headers (e.g. `phys_core.hpp`) alone compile but fail to link: include the umbrella.
- Under `run.sh`, a failing default GPU triggers the llvmpipe retry, so logs can show two runs.
- `P11_REC` opens+closes its file per frame: a FIFO must be held open (`exec 3>fifo`) or ffmpeg exits after frame 1 and the recorder blocks (m2-video.sh does this).
- Reference video: `/run/media/simon/Media/Downloads/2026/Sept - Oct/Spy Vs Spy (PS2) Full Game.webm`.
- `.mdf` is a Russian fan translation — use the ISO. `5C35DB16C6D45BCC` is the MP carnival, not mission 2.
- Model space z-up facing +y; engine = 0.768 · (−x, z, y).

## Key files (in work/friend-SpyVsSpy)
- `AGENTS.md`, `MandateFromHeaven.md` (Works / Doesn't work / Claims), `docs/MISSION2.md`.
- `poc/11_gameplay/{main.cpp,engine/,campaign/,missions/}` (+ README in each); `src/` (+ README).
- Ours: `notes/work-log-2026-10-22.md`, `tools/m2-video.sh`.

## History
- 2026-10-22 (eve): wave r — restructure, agent docs, quiet tests, sky/void check, app loop part 1 ([work log](notes/work-log-2026-10-22.md)).
- 2026-10-22: m2c wave — walkable Carnival, enemy models, mini-games, shop, lighting; `mission-2c` pushed.
- 2026-10-21 (eve): mission-2b — HUD, weapons, Mission 2 v1 (merged by Tyson, PR #4/#5).
- 2026-10-21: mission-2 — animated spy model, Kooky Carnival from the menus (PR #2).
- 2026-10-19: menus → Mansion in the friend's engine (merged into `mission-1`).
- 2026-10-12: vicious matches PS2; 710/710 files decode; repo public ([work log](notes/work-log-2026-10-12.md)).
- 2026-10-05..08: DATA.PAK extractor, format tools, Rust prototype, Ghidra + EE plugin.
