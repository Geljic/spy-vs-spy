# UI / menus — how the game builds its menus

2026-10-19. Same text as `docs/UI.md` in the friend's C++ engine (github.com/LuckyDucko/SpyVsSpy, PR #1,
branch `ui-menus`), where the implementation lives (`src/ui/`, `poc/12_menu`, `poc/13_menu_vk`). Local clone:
`work/friend-SpyVsSpy`. Ghidra names for the functions below are saved in `work/ghidra_proj`. Script dumps:
`tools/script_print.py` on vcs2json JSON of `work/xiso` (the English ISO; `work/x` is the Russian .mdf).

Legend: ✅ read in the ELF and checked on the data/render · 🟡 inferred from data, not yet read in code · ❓ unknown.

## 1. Where the menus are

- **`Maps/293A370B2A0BF3AD.map` (MainMenu)** holds all 46 front-end Dialogs ✅: `DG_Title`,
  `DG_MainMenu`, `DG_SinglePlayer`, `DG_SelectStorySpy`, `DG_SelectMap`, `DG_Multiplayer`,
  `DG_GameSetUp`, `DG_Options*` (Audio, Music, VideoMod, Game, AI, Network, Weapon), `DG_Extras`,
  `DG_WatchMovies`, `DG_ConceptArt`, `DG_Cheats`, `DG_Credits`, `DG_Keyboard`, `DG_SpyFiles`,
  `DG_ModASpy`, `MB_OK`, `MB_YESNO`, `Net_Lobby*`, … (`poc/12_menu/run.sh list`).
- **`Games/0000000100000001.gam`** holds only the shared/system dialogs (`DG_Startup`, `DG_PausesGame`,
  `MB_YESNO`, `DG_MemCardOp`, `Net_*`), the same 4 fonts and the 28 enumerations (`EnumLevelsSpy`,
  `EnumGameMode`, `MsgObjective`, …). **You don't need to hand-dump the .gam:** the vendored decoder
  (or `vcs2json --ps2 --types`) gives every object with field names.
- **Text** is not in the map: `UIText` has a `stringTable` reference + `stringId`; the tables are in
  the language `.loc` files: `Maps/<lang>/293A370B2A0BF3AD.loc` (48 tables: `MainMenu`, `GameSetUp`,
  `Levels_Names`, `NetworkLobby`, …) and `Games/<lang>/0000000100000001.loc` (`MsgBox`, `GameMisc`, …).
  `stringId = (unique << 16) | index`; `String.id = {index, unique}` ✅. Strings are Latin-1 and index
  the font atlas directly.
- ⚠ Simon's own disc dump (`Spy vs. Spy.mdf`) is a **Russian** fan translation: its `eng` .loc files are
  Russian written with Latin letters that the patched font draws as Cyrillic; 81 files differ (all maps,
  .ls, .gam, eng .loc), but only in texture pixels and strings — the decoded trees are identical. Our
  ISO (`DATA.PAK` sha256 `a5d5b1d7…`, = `manifest/`) is the real English one.

## 2. The objects

Field names are vicious's (`f_0xNN` = vicious's guess of the Xbox object offset). The **PS2 object
offset is +4** for UIElement-derived classes; both are given so you can follow the ELF.

| Class (id, PS2 loader) | What | Fields we use |
|---|---|---|
| Dialog (129, `0x1ab310`) | a menu screen | `grid` (root UIGrid), `dimension` (w, h; 0 = fit content), `f_0x50` placement 🟡, `f_0x54/58` offset 🟡, `sprites`, `events` |
| UIElement (132, `0x1b3cd8`) | base of all UI | `f_0x20` flags (+0x24), `f_0x48` **alignment** (+0x4c), `f_0x4c/4e` **offset** x/y in 1/32 px (+0x50/52), `f_0x50..56` **margins left, bottom, right, top** (+0x54..5a) ✅ |
| UIGrid (133, `0x1b7390`) | table layout | `f_1` **rows**, `f_2` **cols** ✅ (`SetGridSize(rows, cols)` `0x1b7988`, cells row-major), `cells[]` (each: first element + more, they overlap), `gridFlags` (+0x7c: `0x10` = fixed size), `f_0x90/94` fixed/min size (+0x94/98) ✅, `f_0x7c/7e` (+0x80/82) ❓ |
| UISelectionGrid (134, `0x1ba058`) | a UIGrid with a selection | as UIGrid |
| UISprite (513, `0x1a9b38`) | image box | `spriteFlags` (+0x7c: `4` = size from the sprite ✅), `width/height` (+0x8c/90), `stateSprites` (3: normal, focused 🟡, disabled 🟡), `opacity` |
| UIText (512, `0x1b12f0`) | text (a UISprite + text) | `stringTable`/`stringId`, `font` (+0xc4), `f_0xa4` **text alignment** (+0xa8), `f_0xa8..ae` padding left, bottom, right, top (+0xac..b2) 🟡 order, `f_0xb0` normal / `f_0xb4` focused / `f_0xb8` disabled 🟡 / `f_0xbc` shadow colour, **ARGB** ✅ (title text `0xffab0000` renders the known red), `f_0xc4` wrap width (+0xc8), `textFlags` (+0xca) |
| Font (64, `0x213b68`) | bitmap font | `material` → SpriteMaterial → `albedo` ImageTexture (256×256, 16×16 cells, cell = Latin-1 byte), `sizeScale` (+0x4c), `glyphWidth/Height` (+0x50/54), `f_0x40 & 1` proportional (+0x44), `f_0x154` baseline (+0x158), `glyphs[256]` = `{yOffset, bearing, width, spacing}` signed bytes (+0x15c) ✅ |
| Sprite (61, `0x2334a8`) | a texture rectangle | `material`, `f_0x44/48` on-screen w/h (+0x48/4c) ✅, `f_0x50..5c` texture pixel rect x0, y0, x1, y1 (can be mirrored) 🟡 |

Textures: every PS2 texture is 8-bit indexed, GS-swizzled, CSM1 palette, alpha ×2 (the `format` byte
doesn't change that) — `assets::unswizzle8` + `assets::csm1` from `src/assets/texture.hpp` decode them.

## 3. Layout rules (all ✅ from the ELF unless marked; checked against the reference video, §8)

The game's UI space is **y up**; `src/ui` works in screen pixels (640×480, y down) and converts.

- **Alignment** (`0x2335b8`): 1 2 3 = top row on screen, 4 5 6 = middle, 7 8 9 = bottom (y up: 7 8 9
  use the rect's minimum y); columns left/centre/right; 0 = no anchor. Same codes for elements and
  text. Proof: Spy Files (root alignment 7) sits bottom-left in the video at 1:52.
- **Measure** (`0x1b5798` base, `0x1aa6e0` sprite/text, `0x1b9d58` + `0x1b9b08` grid):
  size = margins + content (+ frame). Hidden elements (shown bit off) take no space.
  - Margins, object `+0x54..+0x5a` (vicious `f_0x50..f_0x56`) = left, top, right, bottom on screen.
  - Grid content: each column as wide as its widest element, each row as tall as its tallest; an
    empty cell counts as the grid's own margins. Content = Σ columns × Σ rows; with `gridFlags & 0x10`
    it is the fixed size `f_0x90/94`, otherwise that is a minimum.
  - Sprite/text content = max(sprite size, text size); sprite size = state sprite's w/h if
    `spriteFlags & 4`, else `width/height`.
  - **Frame**: a grid with flag `0x100` and a 9-slice list `f_0x8c` (UL UM UR ML MM MR BL BM BR:
    `MNU_Simple_*` title tabs and button bars, `SelGlass_*` lists) grows by (BL.w + BR.w, UL.h + BL.h)
    and its content is inset by the corner.
  - Two global paddings (`DAT_00433b60..6c`) are added too; nothing writes them (bss) → 0.
- **Place** (`0x1b5b30`): align the measured box in its cell rect, add the offset (1/32 px, **y up**),
  the margins and the frame corner → the content box. Grid rows run top→bottom, columns left→right.
- **Shown / enabled**: element flag `0x2` = shown (V148, `0x1b5340`), `0x1` = enabled (V147,
  `0x1b4ed8`); a hidden/disabled parent hides/disables its children (`0x1b5398`/`0x1b4f30`). Disabled
  text uses the disabled colour. The dialog always draws its root grid.
- **Dialog on screen** (`0x1af628`): the root grid is placed by **its own alignment** inside the
  TV-safe area (`0x26a158`: x 48, y 36, 544×408; dialog flag `0x1000` = whole screen), moved by the
  dialog offset `f_0x54/58`. Pop-ups (`0x800`) use the whole screen and add their `f_0x6c` frame.
  `dimension` plays no part. (Main menu at x 48 + frame 12 ≈ the video's ~60 px.)

## 4. Text (font engine `0x214710`, line widths `0x215358`, glyph `0x2144c0`)

- Line height = max(glyphW, glyphH) × scale; text height = (lines − 1) × line height + glyphH × scale.
- Proportional fonts (all four `BodyText*`): advance = (bearing + width + spacing) × scale, bearing
  skipped on the first glyph of a line; line width rounded up. Monospace: glyphW × scale.
- Glyph quad: a whole atlas cell (glyphW × scale square) at pen.x (+ bearing), top =
  line top − (baseline + yOffset) × scale. UV = cell `(c & 15, c >> 4)` / 16.
- Lines break at `\n` and the two-character `\n`; `\t` is ×4 with flag 8. Word wrap at the wrap width
  when `textFlags & 0x08`. `textFlags & 0x10` = drop shadow: shadow colour at (+1.5, +1.5) under the
  text. `~` and `^` are runtime fields (difficulty, player names…) — drawn as-is for now.
- Each line is aligned left/centre/right inside the text block by the text alignment's column.

## 5. Using it

```sh
poc/12_menu/run.sh                         # build + self-test (ci) -> PASS, writes work/poc12/DG_*.ppm
poc/12_menu/run.sh list                    # 46 dialogs
poc/12_menu/run.sh dump DG_MainMenu        # tree, boxes, text
poc/12_menu/run.sh render DG_MainMenu work/poc12/mm.ppm SinglePlayer   # with focus colour
P12_BOXES=1 poc/12_menu/run.sh render DG_MainMenu work/poc12/boxes.ppm # layout outlines
```

In C++: `ui::Library lib; lib.load(map, {map_loc, gam_loc}); ui::Dialog* d = lib.dialog("DG_MainMenu");
ui::layout(*d); auto quads = ui::draw_list(*d, focus);` — each `ui::Quad` is a textured screen rect
(texture RGBA, UV rect, ARGB modulate). A Vulkan pass draws the same list (one textured-quad pipeline,
alpha blend, nearest or linear sampling).

Any other class: `vfile::File f; f.load(path);` then `f.all_of(classId)` or `f.get(ref)`; the structs are
in `third_party/vicious/include/vicious/generated.h`. This replaces hand-written byte walkers.

## 6. Menu flow: the game's own scripts (`src/ui/script.hpp`)

The front end is entirely script-driven; `ui::Runtime` runs those scripts. `poc/12_menu/run.sh flow`
drives it with scripted input and ends in the game's own map change:

Title (`Map.onMapStarted` opens DG_Title on the system layer, sends `Msg_MainMenu.Show_Title` →
`Mgr_Scene` state Title → its Timer runs `StartupCheck`, which opens DG_SpyFiles under the title) →
Start → memory-card check (`DG_MemCardOp`, virtual card) → Spy Files → Load → "Load successful" →
DG_MainMenu → Single Player → Story (`GameMode = Story`, `MM_CurrentMapSelect = Mansion_StoryMode`) →
Select Spy → Select Map (`List_Maps` confirm → `Msg_MainMenu.Begin_StoryGame` → `Mgr_Scene` closes all
dialogs, sends `Msg_AIMainMenu.Start_StoryGame` → menu spy `HL_SpyMainMenu` state Load_StoryGame →
StartGame → `LoadMap_Spy(MM_CurrentMapSelect)` → `ChangeMapStatement` → `Game.maps[GetGameMapIndex()]`
= index 26 = **Mansion_Story 4DC0D0DAD36EB8EF**). On an empty card the game asks "No Spy Files found.
Would you like to create a new Spy File?" (also tested).

Readable dumps of any script: Simon's repo `tools/script_print.py` (vcs2json JSON → pseudo-code).

### 6.1 Language facts (✅ = read in the ELF; addresses are SLES_530.78)

| What | Fact |
|---|---|
| Statement execute / expression evaluate | vtable slot `+0x64` (index 11); assign = slot `+0x7c` |
| AssignValueStatement `0x158168` | op 0 `=`, 1 `+=`, 2 `-=`, 3 `*=`, 4 `/=` ✅ |
| BinaryOperationExpression `0x1bb958` | 0 `&&` 1 `\|\|` 2 `<` 3 `<=` 4 `==` 5 `>=` 6 `>` 7 `!=` 8 `+` 9 `-` 10 `*` 11 `/` (÷0 → 0) 12 min 13 max 14 pow 15 fmod 16 angle diff ✅ |
| UnaryOperationExpression `0x1bd010` | 0 `!` 1 abs 2 floor 3 ceil 4 round 5 sqrt 6 sin° 7 cos° 8 +1 9 −1 10 neg 11 wrap180 15 even 16 asin° 17 acos° 18 x² 19 x³ ✅ |
| TernaryOperationExpression `0x1bc798` | 1 lerp, 2 in range (exclusiveBounds bits), 4 clamp, 8 `a ? b : c`, … ✅ |
| ExecutionContextValueExpression `0x383528` | source 0 = the 2 call/event arguments, 1 = locals, 2 = owner flag ✅ |
| AssetElementIndexExpression `0x383030` | an enum entry's index (`id & 0xffff`); `V421.f_0x10` adds an offset (`EnumLevelsSpy.MainMenu[arg]`) ✅/🟡 |
| OnEnumerationEvent | `f_0x0c` 0 = inline `script`, 1 = global Function `f_0x10` ✅ (data) |
| V138 show dialog `0x1590e8`, load `0x158a00` | flags `f_0x54`: 0x01 open (`0x1af1e0`), else close (`0x1af328`, `f_0x74` → result = V152 prop 1); 0x80\|0x20 close all; layer 0x20 = system, 0x40 = current, else the player's (`0x159078`); `f_0x40` DialogSelector (fallback mode 7 = asset, 1 = the dialog running the script, `0x1564a0`); `f_0x5c` duration, `f_0x64` slide (2 floats), `f_0x74..7c` args (V152 props 0/9/11), `f_0x84` runs on open, `f_0x88` on close ✅ |
| V152 dialog property `0x394c18` | 0/9/11 args, 1 result/state (−1 at open), 2 time open, 4 idle, 7 is open, 5 free; writable 🟡 |
| V194 goto state `0x33b4e0` | `f_0x10` = the new state's timer (×600 ticks), deferred switch (`0x19bd08`) ✅ |
| StateMachine | `f_0x4c` states (V181: handlers V209), `f_0x54` local functions (V200), `f_0x5c` any-state handlers (V218); initial state = `f_0x44.suffix` ✅ (data) |
| V217 handler kind `f_0x0c` | 1 run script `f_0x18`, 2 go to state `f_0x1c`/`f_0x20`, 0 call local function `f_0x10`/`f_0x14` ✅ (data) |
| V331 `0x35d338` | system ops; 1 = bind the controller that pressed Start as player 1, 4 = unbind 1..7 ✅ |
| V253 set text `0x36d268` | mode `f_0x08` 0 table string `f_0x20` (+value), 1 string expression `f_0x24`, 2 selector table, string #value; `f_0x0c == 1` fills `~` slot `f_0x30` ✅ |
| UI events | SystemEvents: UIConfirm 12, UIBack 13, UIShown 14, UIHidden 15, UIFocused 16, UIUnfocused 17, UIUpdateUI 18, UIUpdate 19, UISelectionChanged 21 ✅ (data) |
| Focus / bubbling | focusable = flag 0x400 + shown (0x2) + enabled (0x1); confirm/back go into the focused element's subtree (grids forward to cells, `0x1b87a8`), then up to the dialog; non-zero return stops it 🟡 |
| Selection lists | UISelectionGrid `f_0x98` data source: 515 numeric (V478 3 = clear, 1 = append), 495 profiles, 392 saves; rows get item index as arg 0 (−1 = empty); V497 = selected item 🟡 |
| Memory card | V423 operation → error code; FileSystemStatusExpression property 0 = (error == expected), 1 = card present, 2 = card changed; V438 queries (8 free space, 10 profile count) 🟡 |
| Strings | V378 element text (target mode 9 = focused element), V494 Spy File field (`f_1` index, `f_2` 0 name / 1 net name / 2 shown / 3 file), V379 table string, V370 setting string; AssignStringStatement 384, StringEqualsExpression 492, EditUITextStatement 372 (1 insert, 3 backspace) 🟡 |
| Entity variables | V210 read/assign, per entity-selector mode (`f_6.f_0x00`: 1 = the script's entity, 2 = local player); `VarPlayer.PlayerID` = 1 → Spy File slot 0 🟡 |
| For each child | V466: run its statements once per child of the target, child as self, locals shared (loading-screen challenge names) ✅ (data) |
| Lists | V478 3 = clear, 1 = append value; V497 `f_0x10` 1 = selected item, 0 = item count; scroll arrows greyed via V147 when the list can't scroll 🟡 |
| Sprites | `Sprite.f_0x40` 1/2 = texture rect turned 90° cw/ccw (menu arrows); state sprites 0 normal, 1 disabled ✅ (data) |
| Loading screens | ChangeMap shows `Game.loadingScreens[map index]` (0 = "SPY VS SPY LOADING", 26 = Mansion book); a LoadingScreen's dialog (DG_MM_LoadingBG: 2×2 background tiles in `sprites`) opens DG_Loading; MapLoadStatusExpression 0 = loading, 2 = map being loaded 🟡 |
| Drawing order | a dialog with flag 0x400 (menu screens) hides the 0x400 dialogs below it; backgrounds and screens under a pop-up stay visible 🟡 |
| Navigation | up/down/left/right move to the neighbouring cell of the focused element's grid (shown + enabled), climbing to the parent grid if needed and entering groups at their first focusable element; focusable = 0x400 or reacts to UIConfirm (MB_YESNO's Group_Yes) 🟡 |
| Grid fill | a grid without grid flag 0x1 fills its cell (or the dialog area for the root): main menu list box = title width, keyboard name box, DG_Loading ✅ video |
| Pop-ups | Dialog `f_0x48 & 0x800`: what's under it stays visible; `f_0x6c` = 9-slice frame around the root grid ✅ (`0x1af628`) |
| External variables | `ExternalVariable.f_1` = default value as float bits (GameDifficulty 1 = Medium, as in the video) ✅ |

Not modelled (counted in `Runtime::skipped`, safe for the menus): cameras (V163), animations (V193 → 1 s),
entity relations/objects, sounds (Host::sound gets a tag), network, real memory-card saving (the Host's
virtual card keeps Spy File names in memory). Creating a Spy File works (keyboard or `Runtime::type_text`).

## 7. Not done / next

1. **3D backdrop** (poc/13 draws the MainMenu room `high_res_hideout` behind the menus, cameras from the
   scripts' `Cam_*` points, matching the video's framing): still missing the spy and prop models
   (DynamicMesh), the game's lighting (ours is brighter/flatter) and camera easing; the title folder
   (Cam_TitleShot) still renders upside down. Two findings for the rest of the engine: the level mesh
   must be placed by its `StaticInstance`/`Model` transform (MainMenu: (−12.97, 2.80, 14.03) m; may
   explain Mansion_Story's "no floor at Spy_HumanPlayer"), and poc/09's `1 − v` texture flip looks wrong
   (file UVs as-is fix the folder, notes and poster) — check poc/09/poc/11 frames.
2. Small layout gaps vs the video: the loading screen's challenge texts sit ~13 px left (over their icons);
   the cursor "|" in the name field; the focused key highlight on the keyboard.
3. Save Spy Files to disk (a real Host memory card), and the boot memory-card check at power-on (0:12).
4. Read the 🟡 rows in code (dialog placement, frame rule, state sprites, padding order).
5. Options / Multiplayer / Extras dialogs: they run but their engine hooks (video modes, network) are stubs.

## 8. Reference video (the user's PS2 capture "Spy Vs Spy (PS2) Full Game", 640×360, 3:39:02)

Frames for the menus are extracted to `work/vidref/menu/` (gitignored; `ffmpeg -ss T -i VIDEO -frames:v 1`).
The capture stretches the 4:3 picture to 16:9: scale our 640×480 frames to 640×360 to compare.

| Time | What | Our counterpart |
|---|---|---|
| 0:12 | "Checking memory card (PS2) in memory card slot 1…" pop-up (boot check) | `Game.onGameStarted` → DG_LockFileCheck / DG_MemCardOp BootCheck (not run yet) |
| 0:14 | "SPY VS SPY / LOADING…" loading screen | LoadingScreens/35790767A74102A0.ls (MainMenu) |
| 0:16–1:44 | publisher logos, intro movie | Movies/*.mov (poc/05) |
| 1:46 | title: "PRESS START BUTTON TO BEGIN" on the folder (3D, in the room) | DG_Title (text ✓ position) |
| 1:50–2:10 | Spy Files (bottom-left), "Enter Spy Name" keyboard, "Would you like to save your profile?", "Saving data…", "Save successful" | DG_SpyFiles ✓, DG_Keyboard (not yet), MB_YESNO ✓, DG_MemCardOp ✓, MB_OK ✓ |
| 2:11 | main menu panel over the 3D room | DG_MainMenu ✓ (frames, position, text) |
| 2:13 | "SINGLE PLAYER: MEDIUM" | DG_SinglePlayer ✓ (default difficulty from ExternalVariable) |
| 2:14 | "SELECT SPY" (Black spy / White spy), closet opens | DG_SelectStorySpy ✓ (3D: HL_SpyMainMenu TrophyCase) |
| 2:20–3:30 | story intro cinematic (beach, balloon) | Cinematics/*.cin (not yet) |
| 3:34 | Mansion loading screen: book page, "Chandeliers / Suits of armor / Fall-away bridges / Security gauntlet", LOADING… with a bomb fuse | DG_Loading (text ✓; book background + challenge names V466 not yet) |
| 3:36 | "TELEGRAM FROM SPY EMBASSY" briefing | in-game (Mansion_Story scripts) |
