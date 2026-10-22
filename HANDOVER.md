# Handover: spy-vs-spy

Updated 2026-10-22 (night) after the "p" wave: Mission 2 friend playtest kit (Linux + Windows, gamepad,
bug snapshots, replays) and online play v1 through the ZimaOS relay. Tags `m2-playtest-1`, `m2-playtest-2` pushed.

## Start here (paste as the first prompt of the next session)
> Continue spy-vs-spy (projects/spy-vs-spy). Read CLAUDE.md, HANDOVER.md and notes/work-log-2026-10-22.md
> ("Night"). In `work/friend-SpyVsSpy`: `git fetch` — did Tyson push or merge `m2-playtest` / `m2-net`? Ask Simon
> how his own window playtest went and whether the playit tunnel + Windows zip link exist. Then "Next" from 1.

## Team / split
- Tyson (LuckyDucko, repo owner; repo is private) splits by mission; **Simon owns Mission 2** + the shared HUD.
- Claims in `MandateFromHeaven.md` §4.5 (`[claimed by X @ date]`, push first). Ours: restructure, app loop,
  M2 map polish, **playtest kit**, **netcode**. Tyson's priority: seamless map, never sky below the spy.

## State (verified 2026-10-22, Linux, RTX 3070)
- Branches on origin: `m2-playtest` @ 46fbdb5 (kit + void fixes), `m2-net` @ c9c4219 (= m2-playtest + online).
  Tags: `m2-playtest-1` = 2b7f35d (kit, before net), `m2-playtest-2` = 3e5331a (online v1). ci ALL PASS (17 suites
  incl. `tools/net_test.sh`), `run.sh m2` PASS 86 ok, `run.sh test` PASS 61 ok.
- **Window**: `run.sh launch` opens and plays (book page, briefing, hideout, HUD) — screenshots only, no human yet.
- **Map**: route void spots 0; whole-map survey 35 cells (`M2_VOID_MAX=35`): rock-ridge rims, overhang singles,
  goal-room exit door. Laser tunnel, lifts, tilt-a-whirl, maze trapdoors drawn from the map's own models + solid.
- **Controls**: keyboard/mouse + gamepad (Xbox layout, untested on a real pad). Space now jumps. R / Back respawn.
- **Friend kit**: `./play-m2.sh [--room w --player 1|2 --relay h:p]`; Windows `play-m2.bat [-Room -Player -Relay]`
  (exe + launcher, no Python). Session folder = session.log + input.rec (`P11_REPLAY`) + bugs/ (F9 / RS-click).
  docs/PLAYTEST.md, docs/WINDOWS.md. Windows CI artifact green; zip staged at `dist/SpyVsSpy-M2-playtest-2-windows.zip`.
- **Online v1** (docs/NET.md): own mission per player, spies synced 20 Hz, hits go to the victim's health,
  "PLAYER 2 JOINED/LEFT". Relay = existing Cel Damage relay on ZimaOS TCP 7788 (sessions `svs:<room>`),
  reachable on Tailscale `100.113.42.52:7788` only. Linux ↔ Windows exe (Wine) tested through it.

## Next (priority order)
1. **Simon plays it** in a window (`./play-m2.sh`, then online with `--room`): first human test of controls, feel, F9.
2. **Public relay**: playit.gg → add TCP tunnel → local 7788 (only Simon can); put the host:port in PLAYTEST.md + default.
3. **Share the Windows zip** (ask Simon where: `publish-private` or a link), tell Tyson about `m2-playtest`/`m2-net` (PR?).
4. **Big-top SE slot**: real 0.6 m hole between the big-top wall and terrain, only safe because the deck rail holds.
5. Online v2: shared enemies/mission (host owns state), remote hit reactions on the shooter's side, lag-compensated hits.
6. App loop crash (2nd mission in one process; see campaign/app_loop.hpp), mover models, remaining 35 void cells.

## Unverified / known broken
- No physical gamepad test; nothing on real Windows hardware (only CI + Wine); no human play session yet.
- Tilt-a-whirl coal pit kill is a MADE UP rule (the map's trigger isn't decoded).
- Pad Start acts as a screen's △ only; Esc (keyboard) is the only quit.
- `p-void2` survey changes are justified by sampled sim walks, not proven for every cell (3-cell overhang pocket unclear).

## How to verify
```sh
cd projects/spy-vs-spy/work/friend-SpyVsSpy
ci/build_test.sh                          # ALL PASS (17)
poc/11_gameplay/run.sh m2                 # PASS m2: 86 ok, 35 void cells allowed
tools/net_test.sh 100.113.42.52:7788      # PASS net: 8 ok via ZimaOS
tools/windows/cross_check.sh              # PASS win-cross (zig + Wine)
P11_NET=100.113.42.52:7788,t1,1 P11_NETBOT=1 work/poc11/bin launch 692B67BD109F597F BCA0DBB9A22F1441 x.ppm hidden &
P11_NET=100.113.42.52:7788,t1,0 work/poc11/bin launch 692B67BD109F597F BCA0DBB9A22F1441   # white spy walks
```

## Gotchas
- No audio ever; visible windows OK. `work/poc11/bin` is per checkout: one `run.sh` at a time per worktree.
- Worktrees `work/wt/p-{void,input,win,net,void2}` (all merged); data symlinked. Never `git add -A` (deps symlink).
- windows.h breaks the single TU (`Play`, `BOOL`): OS code goes in its own .cpp (see `src/net/sock_win.cpp`).
- MSVC can't build (vicious zero-size structs) — MinGW only.
- `.mdf` is a Russian fan translation — use the ISO. Model space z-up; engine = 0.768 · (−x, z, y).

## Key files (in work/friend-SpyVsSpy)
- `docs/PLAYTEST.md`, `docs/NET.md`, `docs/WINDOWS.md`, `docs/MISSION2.md`, `MandateFromHeaven.md`, `AGENTS.md`.
- `src/net/`, `engine/{gamepad,session,hint_text}.hpp`, `play-m2.sh`, `tools/windows/`, `CMakeLists.txt`.

## History
- 2026-10-22 (night): playtest kit, Windows build, online v1, voids 592 → 35 ([work log](notes/work-log-2026-10-22.md)).
- 2026-10-22 (eve): wave r — restructure, agent docs, sky/void check, app loop part 1.
- 2026-10-22: m2c wave — walkable Carnival, enemy models, mini-games, shop, lighting.
- 2026-10-21: mission-2b (HUD, weapons, M2 v1) and mission-2 (animated spy, Carnival from the menus).
- 2026-10-19: menus → Mansion in the friend's engine. 2026-10-12: vicious matches PS2; repo public.
- 2026-10-05..08: DATA.PAK extractor, format tools, Rust prototype, Ghidra + EE plugin.
