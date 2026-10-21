# Handover: spy-vs-spy

Updated 2026-10-21 (evening) after the mission-2b wave: in-game HUD, weapons, and Mission 2 playable start to
finish with stand-ins (branch `mission-2b` in `work/friend-SpyVsSpy`, commit `c6fdd32`, pushed to LuckyDucko/SpyVsSpy 2026-10-21). Parked by Simon after the push.

## Start here (paste as the first prompt of the next session)
> Continue spy-vs-spy (projects/spy-vs-spy). Read CLAUDE.md, HANDOVER.md, then in `work/friend-SpyVsSpy` read
> `docs/MISSION2.md` ("Built (v1)") and `docs/NOTES.md` §9 (last 4 lines). Ask Simon whether `mission-2b` was pushed /
> merged by Tyson; `git fetch` and rebase on Tyson's `mission-1` if he merged his loading/briefing fixes.
> Run `ci/build_test.sh` (ALL PASS) + `poc/11_gameplay/run.sh m2` (PASS), then work down "Next" from 1.

## Team / split
- Tyson (LuckyDucko, owner) splits by mission; **Simon owns Mission 2** (Kooky Carnival, video 18:51–33:40).
  Tyson merged our `mission-2` (PR #2) into `mission-1` on 2026-10-21.
- Tyson 2026-10-21: the in-game HUD owner is "whoever makes it work" → **us** (shared with Mission 1). His agent is
  fixing the loading book page + briefing telegram ("a lil off") — don't touch `UiOverlay` states 1–2.
- Rules: `poc/11_gameplay/main.cpp` mission-agnostic; per-mission content in `poc/11_gameplay/missions/`; shared
  code in `src/`; `helpers/` read-only; **status lines in `docs/NOTES.md` §9 tagged `[OS, agent]`** (Tyson asked
  that every agent writes its line); commit as Simon (noreply email).

## State (verified 2026-10-21, Linux, RTX 3070, headless only)
- **Shared contract** `src/game/state.hpp` (`game::State`: money/Bux/kills/health, item row = `Item_Names` index,
  keycard, HUD flags, objective/message/popup, boss bar, victory, actors, hits, props) + `Mission::on_start/on_tick`
  + `Play` (`src/mission.hpp`). Input: E interact, Q / wheel cycle weapons.
- **HUD** `src/ui/hud.hpp`: the maps' own dialogs (`HUD_Health`, `HUD`, `HUD_BossHealth`, `DG_GenericText`,
  `DG_VictorySingle`) with real `Item_*`/keycard/boss sprites; matches the video within a few px (docs/UI.md §9).
- **Weapons** `src/actor/weapon.hpp` + `actor::UpperBody` (src/actor/spy.hpp): `club`/`slingshot`/`gun` models on
  `mh_rhand`, AnimPb* upper-body clips over locomotion, club melee, slingshot ricochet, machine gun with ammo; enemy
  spies hold/fire/flinch/die (docs/MODELS.md "Weapons").
- **Mission 2 v1** `missions/carnival2{,_play,_test}.hpp`: 4 phases (keycard → barricades → plans → clown boss →
  cannon → victory), bean bag / bell / gallery as stand-ins, 28 minions + boss, death −10 %, hideout exit fixed.
- Checks: `ci/build_test.sh` ALL PASS; `run.sh test` (Mansion + M2 launch + hud + weapons) PASS; `run.sh m2` PASS.
- Our repo (github.com/Geljic/spy-vs-spy): HANDOVER + work log committed locally, not pushed.

## Next (priority order)
1. **Tell Tyson** (`mission-2b` pushed 2026-10-21): HUD is shared — Mission 1 should call `st` the same way
   (money/health/items/objective) instead of building its own.
2. **Visible playtest** (`./play.sh` → Kooky Carnival) — nothing has been played in a window yet; tune feel,
   camera, weapon rates (all made up, commented in weapon.hpp / carnival2_play.hpp).
3. **Collision islands**: wrench platform, maze, west park, fun-house interior are separate walkable regions in our
   collision → bridged by cyan "travel posts". Find why (missing stairs/ramps, instance placement?) with
   `P11_M2FLOOD` / `P11_M2COMP` (carnival2.hpp).
4. Real minion/boss models (`bumpmonkey`, `ClownCar` templates; actors are boxes now); victory input (△/✕).
5. Missing content: Kooky Cook + pizza ride, bells 2–3 unlocks, shop screen (`DI_Purchase`), clown wheel sting,
   slingshot band clips, Carnival lighting.

## Unverified / known broken
- Nothing run in a visible window or on macOS. Bean-bag booth renders as a grey wall; fun-house cage corridor black.
- White spy coat shows dark blotches (`Spy_White` material set) — pre-existing.
- HUD overlay is CPU-rasterized every frame (~6 ms).

## How to verify
```sh
cd projects/spy-vs-spy/work/friend-SpyVsSpy
ci/build_test.sh                  # ALL PASS
poc/11_gameplay/run.sh test       # Mansion walk + M2 launch shots + hud + weapons
poc/11_gameplay/run.sh m2         # Mission 2 scripted playthrough, 46 ok lines
```

## Gotchas
- Loading a second vicious file late (after another `vfile::File` was freed) segfaulted: load in `UiOverlay::init`.
- `P11_WTEST` (weapons test) resets `st` and skips the mission hooks, else Mission 2 rules kill its dummy.
- Agent worktrees `work/wt/{weapons,hud,m2}` (branches `m2b-*`, merged) hold their frames; data is symlinked;
  delete with `git worktree remove --force` when done. Shared `work/poc11/bin` crashes concurrent builds.
- Weapon templates are lower-case (`club`, `slingshot`, `gun`); no `Wpn_Club`. Mission 2 starts with the slingshot.
- `.mdf` is a Russian fan translation — use the ISO. `5C35DB16C6D45BCC` is the MP carnival, not mission 2.
- Model space z-up facing +y; engine = 0.768 · (−x, z, y).

## Key files (in work/friend-SpyVsSpy)
- `src/game/state.hpp`, `src/mission.hpp`: gameplay contract.
- `src/ui/hud.hpp`, `src/actor/weapon.hpp`, `src/actor/spy.hpp`: HUD, weapons, spy + upper body.
- `poc/11_gameplay/missions/carnival2*.hpp`: Mission 2. `docs/MISSION2.md`, `docs/UI.md` §9, `docs/MODELS.md`.
- Ours: `notes/work-log-2026-10-21.md` (both sessions).

## History
- 2026-10-21 (eve): mission-2b — HUD, weapons, Mission 2 v1 playable ([work log](notes/work-log-2026-10-21.md)).
- 2026-10-21: mission-2 — animated spy model, Kooky Carnival from the menus; merged by Tyson (PR #2).
- 2026-10-19: menus → Mansion in the friend's engine (merged into `mission-1`).
- 2026-10-12: vicious matches PS2; 710/710 files decode; repo public ([work log](notes/work-log-2026-10-12.md)).
- 2026-10-05..08: DATA.PAK extractor, format tools, Rust prototype, Ghidra + EE plugin.
