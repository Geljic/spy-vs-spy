# Handover: spy-vs-spy

Updated 2026-10-22 after the "m2c" wave: five agent branches merged into `mission-2c` (Kooky Carnival
walkable end to end, real enemy models, real mini-games, shop, baked lighting), pushed to LuckyDucko/SpyVsSpy.

## Start here (paste as the first prompt of the next session)
> Continue spy-vs-spy (projects/spy-vs-spy). Read CLAUDE.md, HANDOVER.md and notes/work-log-2026-10-22.md.
> In `work/friend-SpyVsSpy`: `git fetch` and check whether Tyson pushed to `mission-1` or merged `mission-2c`
> (his agent was paused 2026-10-22). Run `ci/build_test.sh` (ALL PASS) + `poc/11_gameplay/run.sh m2` (PASS),
> then work down "Next" from 1.

## Team / split
- Tyson (LuckyDucko, repo owner, good friend; plain messages are fine) splits by mission; **Simon owns
  Mission 2** (Kooky Carnival) and the shared in-game HUD. Tyson's local agent: 260k context, paused 2026-10-22.
- 2026-10-22 Tyson agreed: **we run the core-engine restructure** (Mission 1 must keep working; pull
  `mission-1` first). **Claims** go in `MandateFromHeaven.md` as `[claimed by X @ date]` next to each
  "doesn't work" item, first push wins, clashes sorted by talking. Fresh agent flow: pull → read open
  items → ask what to do → claim + push → split big work into small claims first.
- Tyson's priority from the video: **seamless map over mini-games; never sky/void below the spy.**
- Rules: mission-agnostic engine, per-mission `poc/11_gameplay/missions/`, shared `src/`, `helpers/`
  read-only, a `docs/NOTES.md` §9 line `[OS, agent]` per agent, commit as Simon (noreply email).

## State (verified 2026-10-22, Linux, RTX 3070)
- Branch `mission-2c` (pushed) = Tyson's `mission-1` @ e4d006f (offscreen headless, engine split part 1)
  + 11 commits: our five merged branches + `P11_REC` video recorder + doc fix.
- Checks on HEAD: `ci/build_test.sh` ALL PASS 16/16; `run.sh m2` PASS (~80 ok lines, walks the whole route);
  `run.sh test` PASS (Mansion walk, launch shots, hud, weapons, actors). Frames looked at by eye.
- **Walkability**: no travel posts; lifts, laser tunnel, doors are movers; terrain collision fixed (was 7.68 m off).
  Door API `coll_set_door` / `coll_set_trapdoor` (missions/carnival2_coll.hpp).
- **Enemies**: map's own animated models (`src/actor/creature.hpp`, missions/carnival2_actors.hpp).
- **Mini-games**: bean bag / bell / gallery as real screens with the maps' dialogs; rules from the map's scripts
  (`engine/minigame.hpp`, missions/carnival2_minigames.hpp).
- **Flow**: shop (`DG_Purchase`, prices from `VarItem_Costs`), clown wheel sting (`st.cut()`), bells open
  their doors (own `State::open_doors` + stand-in boxes), victory ✕/△ ends the mission (`State::end`).
- **Look**: baked per-vertex lighting, sky dome round the camera, static entity props (booth, cages,
  spotlights), White spy coat fixed (texture V flip). Carnival only (`Mission::prelit`, `static_props`).
- Video for Tyson: `dist/spy-vs-spy-m2-poc.mp4` (43 s, scripted run at 3×; old pre-coll/look build).

## Next (priority order)
1. **Map polish (one agent)**: draw the map's own models for movers (lifts are grey boxes, cyan door/marker
   boxes) instead of stand-ins; fix the void spots coll listed (roller-coaster track collision ~1 m off the
   render, west park x −66..−55, tunnel floor at (21, −41.5), maze pit edges; full list in
   docs/MISSION2.md "Walkability"); a frame check that fails on sky below the spy; tilt-a-whirl pit (no
   collision in its template); reconcile flow's door boxes with `coll_set_door`.
2. **Core restructure (one agent, after 1)**: one program for menus + gameplay with an app loop (menu →
   loading → mission → victory → menu, replaces play.sh, finishes flow's menu return + story progress);
   `engine/` (render, physics, assets, window) / campaign layer (mission contract, HUD, cutscenes,
   creatures, mini-games, shop, doors) / `missions/`. For Tyson's 260k agent: files ≤ ~500 lines,
   `AGENTS.md` < 80 lines, folder READMEs, NOTES §9 trimmed to ~10 short entries (rest archived), quiet
   tests, Mandate gets per-mission "doesn't work" lists + claims. Mission 1 checks pass at every step.
3. Mission 2 leftovers: helmet placement (`P11_HELMET*` knobs), entity models not drawn (booth/bell
   `ca_strongbell` multi-part rigid bug, ducks, turret gun, wheel `mnky_wheel`, clown), enemy projectiles,
   boss throwing suicide monkeys, Kooky Cook + pizza ride, maze trapdoors.
4. Video playback (disc `.mov`s; poc/05_media decodes them) and the map's own trigger scripts, in the new core.
5. 16:9 after Missions 1 + 2 match the original (3D already any aspect; HUD is a 4:3 layer).

## Unverified / known broken
- Nothing played by hand in a window yet (all headless scripted runs).
- Bell game frame shows the spy over dark void/stand-ins; cyan boxes visible in hideout and big top.
- Look's brightness scale and shader-flag meanings are guesses matched to video frames.
- Victory → menu loop in play.sh not built; `P13_RESUME`/story-progress save not built.

## How to verify
```sh
cd projects/spy-vs-spy/work/friend-SpyVsSpy
ci/build_test.sh                    # ALL PASS (16)
poc/11_gameplay/run.sh m2           # Mission 2 scripted walk-through, PASS; frames work/poc11/m2.ppm.*.ppm
poc/11_gameplay/run.sh test         # Mansion + launch + hud + weapons + actors
# video of a scripted run: mkfifo f; ffmpeg -f image2pipe -c:v ppm -i f out.mp4 & exec 3>f;
#   P11_REC=f,6 work/poc11/bin mtest x.ppm 692B67BD109F597F   (~2.5 frames/s render)
```

## Gotchas
- No audio output in the engine; testing is silent. Simon allows visible windows, never audio.
- Agent worktrees `work/wt/c-*` (branches `m2c-*`, merged) and old `work/wt/{hud,m2,weapons}`: deleting them
  needs Simon's OK (the auto-mode classifier blocked `git worktree remove --force`).
- `work/poc11/bin` is shared: don't run `run.sh` (it rebuilds) while another run uses the binary.
- A null `st.prop("link_*")` deref segfaults silently: travel posts are gone, don't look them up.
- Reference video: `/run/media/simon/Media/Downloads/2026/Sept - Oct/Spy Vs Spy (PS2) Full Game.webm`.
- `.mdf` is a Russian fan translation — use the ISO. `5C35DB16C6D45BCC` is the MP carnival, not mission 2.
- Model space z-up facing +y; engine = 0.768 · (−x, z, y). Weapon templates are lower-case (`club`…).

## Key files (in work/friend-SpyVsSpy)
- `MandateFromHeaven.md` (Tyson's rules), `docs/MISSION2.md` (evidence, Built, Walkability, Not built).
- `src/game/state.hpp`, `src/mission.hpp`: gameplay contract; `src/actor/{spy,weapon,creature}.hpp`.
- `poc/11_gameplay/engine/{sim,overlay,minigame,modelprop,flow}.hpp`; `src/ui/{hud,shop}.hpp`.
- `poc/11_gameplay/missions/carnival2{,_play,_test,_actors,_minigames,_flow,_coll}.hpp`.
- Ours: `notes/work-log-2026-10-22.md`, `notes/work-log-2026-10-21.md`.

## History
- 2026-10-22: m2c wave — walkable Carnival, enemy models, mini-games, shop, lighting; `mission-2c` pushed ([work log](notes/work-log-2026-10-22.md)).
- 2026-10-21 (eve): mission-2b — HUD, weapons, Mission 2 v1 (merged by Tyson, PR #4/#5).
- 2026-10-21: mission-2 — animated spy model, Kooky Carnival from the menus (PR #2).
- 2026-10-19: menus → Mansion in the friend's engine (merged into `mission-1`).
- 2026-10-12: vicious matches PS2; 710/710 files decode; repo public ([work log](notes/work-log-2026-10-12.md)).
- 2026-10-05..08: DATA.PAK extractor, format tools, Rust prototype, Ghidra + EE plugin.
