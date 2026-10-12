# Script VM plan: running the engine's own behaviour scripts

Phase 2 prep, 2026-10-12. This is a survey of the Vicious Engine behaviour/script system, as the vicious schema
describes it and as our PS2 data shows it, plus a plan for a Rust interpreter in `game/`. **No game code was
written in this phase.**

Sources: the vicious schema (`work/vicious/packages/core/src/schema/types/`, MIT, see
[vicious-review.md](vicious-review.md)) for class ids, names and field order, and byte-level scans of our own
extracted PS2 files (`work/x/`) for everything marked ✅. `work/ps2-decode/json/` did not exist when this was
written, so **all usage counts come from our own byte-signature scan** of the PS2 files, not from vicious
(`work/review/scriptcount.py`, `scriptrefs.py`, `graph.py`, `doorsm.py`, `doorscope.py`). Once ps2-decode's JSON
exists, re-count from it and compare.

Legend: ✅ checked on PS2 data · 🟡 likely (name + structure + data hint) · ❓ open.

## 1. How the system is put together

### 1.1 Building blocks
| Concept | Class (id) | What it holds | Evidence |
|---|---|---|---|
| Script | `Script` (105) | v2: one `ExpressionList` chunk (`i32 count` + `count` × `AssetFromType` statements), then an 8-byte `ID` | ✅ 12,834 scripts in 10 maps: count = number of child statements in every one, 8 bytes after the list |
| Statement | subclasses of `Statement` (107) | chunk of the class → `Statement` chunk (`u8 t1` v>1, `u8 t2` v=2) → empty `ExpressionWrap` chunk → own fields | ✅ byte signature found 819,162 script nodes; 99.3% resolve to a known statement/expression id |
| Value expression | subclasses of `ValueExpression` (108); some via `GameStateExpression` (109) | same pattern, one wrapper chunk more for the 109 family | ✅ as above |
| Child expression / statement | `AssetFromType` | `i32 classId` + object; `-1` = none | ✅ |
| Statement list | `ExpressionList` | `i32 count` + `AssetFromType` items (if/else bodies, loops, cases) | 🟡 |
| Global function | `Function` (116) | object (name) + `Script` | ✅ called by `CallFunctionStatement` 11,062/11,063 |
| Global variable | `Variable` (121) | object + `f32 value` | ✅ read by `V157` |
| Constant | `Constant` (123) | object + `f32 value` | ✅ read by `V158` 6,370/6,370 |
| External variable | `ExternalVariable` (357) | object + `u32` (engine-provided value, e.g. `GameMode`) | ✅ read by `V358` 16,549/16,549 |
| Enumeration | `Enumeration` (171) → `EnumerationEntry` (170) | named list of entries (`Named`, `u32 id`, `bool enabled`) | ✅ 77,379 entries as children of enumerations |
| Enum-entry selector | `V421` | `u32`, enumeration ref, `u32 enumerationId`, optional index expression | 🟡 |
| State machine | `StateMachine` (182) | object + parent ref + 3 lists of children: states, local functions, machine-level handlers | ✅ children: `V218` 10,432, `V181` 9,714, `V200` 6,563, nested SMs 391 |
| State | `V181` | `Named` (state name) + `u32` + ref + `u32` + list of `V209` handlers | ✅ Door: `Closed`, `Closing`, `Open`, `Opening`, `Locked` |
| Handler (in a state) | `V209` (= `V217`) | event = enum ref + `u32` entry (`State`/`Msg*`/`SystemEvents`), a second ref + `u32`, a `V255` value, a ref + `u32`, then the `Script` | ✅ keyed by `State` 14,476, `Msg*` 6,927, `SystemEvents` 812 |
| Handler (machine level) | `V218` (= `V217` + 2 ref lists) | same, for messages handled in any state | ✅ keyed by `Msg*` 8,460, `SystemEvents` 1,564 |
| Local function | `V200` | `Named` (function name) + `u32` + `Script` | ✅ Door: `HandleOpening`, `HandleClosing`, `SpawnTrap`, `CheckForCounter`, … |
| Behaviour slot on a template | `V258` (`V154` family) | state machine ref + `EnumerationAnimations` | ✅ `EntityTemplate` → `V258` → `StateMachine` 1,142 |
| Initial variables | `V174` | enumeration ref + `f32[]` (one value per entry) | ✅ refs `Const*` 9,746, `Var*` 5,622 |
| Relation slot | `V306` (`V305` + `V421`) | `Rel*` enum entry | ✅ `Rel*` 17,805/17,805 |
| Event hook on UI/entities | `OnEnumerationEvent` (278) | enumeration + entry id + `Script`/`Function` | ✅ keyed by `SystemEvents` 9,494; parents mostly UI (`UIText`, `UIGrid`, `Dialog`) |
| Other script owners | `Game` (28 `on…` scripts), `Map` (6), `Trigger` (`onEnter`/`onInside`/`onExit`), `Timer` (`onStarted`/`onUpdated`/`onStopped`/`onCompleted`), `V419`, `V380`, `V324` (`onHit`), `V126` (surface `onTouched`), `CinematicFrame` | | schema; ✅ Trigger and Timer scripts seen |

### 1.2 The symbol tables
Enumerations (labels in the data) act as the engine's symbol tables. The prefix gives the role:

| Prefix | Role | Typical users |
|---|---|---|
| `State` (`Enter`, `Exit`, `Update`, `Timer`) | state lifecycle events | `V209` handlers |
| `Msg*` (`MsgDoor`, `MsgGame`, `MsgItem`, `MsgBiped`, …) | message ids | `V192` send, `V209`/`V218` handlers |
| `SystemEvents` (`Activate`, `OnCollided`, `AnimationCompleted`, `PathCompleted`, `UIConfirm`, …) | engine events | handlers, `OnEnumerationEvent` |
| `Var*` | per-entity float variables | `V210` read, `AssignValueStatement` |
| `Const*` | per-template constants | `V212` read, `V174` initial values |
| `Rel*` | per-entity object references ("relations") | `AssignFileReferenceStatement`, `SetContextObjectStatement`, `ObjectExistsExpression`, `ForEachStatement` |
| `Cat*` | categories | `V307` |
| `Gs*` / `GS_*` | global game state | `V436` |
| `Time*` | entity timers | `V272`, `V274` |
| `Anim*` | animation sets | `V193`, `V273`, `V258` |
| `PCtrl*` | player-controller components | `V266`–`V268`, `V271`, `V283`, `EntityComponentExistsExpression` |
| `Enum*`, `Mem*` | plain enums (switch cases, literals) | `AssetElementIndexExpression` (341) |

Mansion_Story alone holds 450 enumerations and 102 state machines (`Door`, `Safe`, `Item_DoorTrap`,
`Item_MoneySafe`, `HL_SpyBase`, `LL_Spy`, `HL_AISpyBase`, …). ✅ `names.py`

### 1.3 Values
- Numbers are **`f32` everywhere**: `Variable.value`, `Constant.value`, `FloatLiteralExpression.value`, the `V174`
  tables. Booleans are floats (0/1); `0.0` and `1.0` are 72% of all literals ✅.
- Object values: entity selectors, relation slots (`Rel*`), file/asset references, strings (`AssignStringStatement`,
  `StringEqualsExpression`).
- `ExecutionContextValueExpression (source, index)` reads the current call context. Pairs seen ✅: (1,0) 42,614,
  (1,1) 14,913, (0,0) 12,687, (1,2) 7,791, (2,0) 6,997, (0,1) 4,279. Probably message/function arguments, the
  sender/self and the return value ❓. The friend's evaluator code will settle this.
- Addressing inside a state machine uses a `u32` that is a `LabelID {u16 index, u16 unique}`. ✅ In Door, the
  `V194` indices are 0–4 (its 5 states) and the `V201` indices are 0–6 (its 7 local functions).

### 1.4 Entity selectors
`EntitySelector` (301, v3 on PS2, 69,630 chunks): `FN_0x22080` (an `AssetFromType`: the expression that yields
the entity), `FN_0x21c40` (3 × `u8` flags + ref) and a `V421` enum-entry selector (typically a `Rel*` slot).
So "the entity" is normally "the object in relation slot X of self", or the result of an expression 🟡. Other
selectors (`PointSelector`, `TimerSelector`, `CameraSelector`, …) are `TypedAssetSelector` subclasses with an
`expression` + `fallback` pair.

### 1.5 Where scripts live in the files
- `.map`: `Map` has `onMapStarted`/`onMapEnded`/`onPlayer…` scripts. Behaviour comes in as `EntityTemplate`s
  (inline on their first entity) with `V258` slots that point at `StateMachine`s, plus `Function`, `Variable`,
  `Constant`, `Enumeration`, `Trigger` and `Timer` objects in their groups. ✅ Mansion_Story: 30,864 script
  nodes; CreepyCarnival: 30,177.
- `.gam`: `Game` with 28 event scripts (UI, cinematics, network, save) and the shared enumerations
  (`Game.enumerations`): 1,414 script nodes ✅.
- Many templates/state machines are repeated in every map file that uses them, so counts over all maps include
  duplicates. Focus counts are Mansion_Story + CreepyCarnival + the `.gam`.

## 2. Usage on PS2
- All 30 files (29 maps + `.gam`): **819,162 nodes**, of which 813,693 (99.3%) map to 140 distinct
  statement/expression classes. 5,469 are unresolved (our signature scan, not the data).
- Focus (Mansion_Story + CreepyCarnival + `.gam`): **62,455 nodes, 133 distinct classes** (70 statement, 63
  expression types).
- Statement and expression classes in vicious that never occur on PS2: 113, 180, 277, 332, 393, 408, 417, 432,
  439, 440, 445, 447, 450, 451, 453, 454, 458, 459, 475, 476, 484, 496, 499, 500, 507 (plus the abstract bases
  107/108/109/343).

## 3. Catalogue of statements and expressions
"Own fields" are the vicious field names (`obj` = `AssetFromType`, `ref` = `AssetReference`, `stmts` = statement
list, `enumEntry` = `V421`). "Uses" come from our scan; "focus" = Mansion_Story + CreepyCarnival + `.gam`. The
"Meaning" column is **our** reading of names + structure + what the references point at; vicious' AI names are
not taken as proof (vicious-review §4.4).

### Statements (89 classes)

| id | Class (vicious name) | Own fields | Uses, all PS2 | Uses, focus | Meaning (ours) | Confidence |
|---:|---|---|---:|---:|---|---|
| 120 | AssignValueStatement | operation:u32, destination:obj, source:obj | 83,910 | 6,410 | destination (op)= source; op 0 in 92% of uses (plain assign); 1–4 likely compound ops | 🟡 op codes ❓ |
| 118 | IfStatement | condition:obj, trueExpr:stmts, falseExpr:stmts | 76,752 | 5,841 | if (condition) then-list else-list | 🟡 |
| 192 | V192 | f_1:entity, f_2:enumEntry/ref, f_0x4c:obj[], f_0x54:obj, f_0x58:u32, f_0x5c:bool | 21,896 | 1,817 | **Send a message to entity**: target entity, message = enum entry (Msg* in 86%), argument list, optional result/flags | 🟡 strong (data: Msg* enums) |
| 119 | ReturnStatement | value:obj | 18,606 | 1,383 | Return a value from the current function/handler | 🟡 |
| 201 | V201 | f_0x08:ref, f_0x0c:u32, f_0x10:obj[], f_0x18:obj | 15,542 | 1,137 | **Call a state-machine local function** (V200) by index, with arguments and result | 🟡 strong (Door: indices 0–6 = its 7 functions) |
| 215 | CaseStatement | body:stmts, matchValues:obj[] | 13,584 | 1,098 | case match-values: body | 🟡 |
| 194 | V194 | f_0x08:ref, f_0x0c:u32, f_0x10:obj, f_0x14:obj[], f_0x1c:obj, f_0x20:bool | 11,304 | 858 | **Change state** of a state machine: SM ref + state index (LabelID) + target | 🟡 strong (Door: indices 0–4 = its 5 states) |
| 115 | CallFunctionStatement | function:ref, arguments:obj[], f_0x14:obj | 11,063 | 822 | Call a global Function (#116) with arguments; `f_0x14` probably receives the result | ✅ ref→Function in 11,062/11,063 |
| 246 | SetContextObjectStatement | slot:u32, object:obj | 9,710 | 734 | Store an object in a context slot (Rel* relation) | 🟡 data: Rel* 6,610/6,610 |
| 312 | AssignFileReferenceStatement | destination:obj, source:obj | 9,230 | 684 | Assign an object reference into a relation slot (Rel*) | 🟡 data: Rel* 8,882/8,882 |
| 253 | V253 | f_0x08:u32, f_0x0c:u32, f_0x10:uiTarget, f_0x1c:ref, f_0x20:u32, f_0x24:obj, f_0x2c:obj, … +2 | 5,166 | 392 | Set UI text from a StringTable/Dialog (mode u32 0/1/2) | ❓ |
| 138 | V138 | f_0x08:camera, f_0x40:dialog, f_0x54:u32, f_0x58:u32, f_0x5c:obj, f_0x60:ref, f_0x64:id, … +7 | 5,136 | 385 | UI/dialog operation on a camera/dialog target (opens/updates a Dialog) | ❓ refs Dialog 2,651 |
| 195 | V195 | f_0x08:entity, f_0x34:u32, f_0x3c:u32, f_0x40:obj, f_0x38:u32 | 4,441 | 392 | Entity operation with two u32 modes and a value (attach/relationship?) | ❓ |
| 224 | SpawnEntityStatement | f_0x16_1:template, f_0x16_2:ref, f_0x20:u32, f_0x34:ref, f_0x24:point, f_0x38:entity, f_0x68:label, … +27 | 3,904 | 342 | Spawn an entity from a template at a point/entity (many options) | 🟡 |
| 214 | SwitchStatement | target:obj, cases:stmts, defaultBody:stmts | 3,153 | 254 | switch (target) { case… } default-list | 🟡 |
| 345 | V345 | f_0x60:u32 | 3,028 | 230 | Spawn-like op from a template list (subclass of V343, extra u32) | ❓ |
| 193 | V193 | f_0x08:entity, f_0x38:enumEntry/ref, f_0x34:u32, f_0x50_1:anim, f_0x50_2:ref, f_0x64:obj | 2,968 | 238 | Play an animation on an entity (Anim* enum entry, custom-animation selector) | 🟡 data: Anim* 2,448/2,463 |
| 346 | V346 | f_0x60:stmts | 2,424 | 180 | Same as 345 but with a statement list (per spawned entity?) | ❓ |
| 249 | V249 | f_0x08:u32, f_1:sound, f_2:ref, f_0x24:obj, f_0x0c:u32 | 2,301 | 170 | Play/stop a sound effect variant on a channel | ❓ refs SoundEffect 2,088 |
| 335 | PlaySoundStatement | channel:obj, target:entity, sound:template, sound_old:ref, stop:bool | 2,286 | 269 | Play (or stop) a sound on an entity | 🟡 |
| 310 | V310 | f_0x08:u32, old:ref, f_0x0c:template, f_0x24:stmts, f_0x2c:(ref+u32)[] | 2,046 | 139 | For each entity of a template: run statement list | ❓ |
| 465 | WhileStatement | condition:obj, body:stmts | 1,705 | 121 | while (condition) body | 🟡 |
| 266 | V266 | f_0x08:entity, f_0x34:u32, f_0x38:enumEntry/ref | 1,634 | 130 | Player-controller (PCtrl*) operation on an entity | ❓ |
| 267 | V267 | f_0x04:entity, f_1:enumEntry/ref, f_2:enumEntry/ref, f_0x64:obj[], f_0x6c:obj | 1,631 | 132 | Player-controller (PCtrl*) operation with two enum entries + args | ❓ |
| 420 | ForEachStatement | source:obj, f_0x0c:ref, body:stmts | 1,517 | 106 | for each object in a Rel* list: run body | 🟡 |
| 245 | BreakStatement | — | 1,428 | 105 | break out of loop/switch | 🟡 |
| 326 | SetCameraFlagsStatement | setMask:u32, clearMask:u32, camera:camera | 1,387 | 116 | Set/clear camera flag bits | 🟡 |
| 506 | V506 | f_1:uiTarget, f_0x14:u32, f_0x18:u32 | 1,356 | 94 | UI target operation (two u32) | ❓ |
| 340 | V340 | f_0x08:enumEntry | 975 | 60 | Operation on an enum-entry target (Gs*/Msg*/Var*) | ❓ |
| 367 | SetUISpriteStatement | target:uiTarget, stateIndex:u32, sprite:sprite | 904 | 84 | Set a UI element's sprite for a state index | 🟡 |
| 281 | V281 | f_0x54:entity, f_0x08:label, f_0x80:camera, f_0xbc:obj, onCollide:script, f_0xb8:u32, f_0xc0:obj, … +19 | 892 | 78 | Spawn a projectile/collider with `onCollide` script | ❓ |
| 308 | V308 | f_0x08:entity, f_0x38:obj, f_0x6c:obj, f_0x70:obj, f_0x3c:entity, f_0x68:ref, f_0x34:u32, … +1 | 858 | 74 | Change an entity's material set / skin (used by Door "ChangeMaterial") | 🟡 |
| 342 | V342 | f_0x08:u32, f_0x0c:entity, f_0x38:entity, f_0x64:label, f_0x7c:obj, f_0x80:obj | 792 | 58 | Entity-to-entity operation with helper label | ❓ |
| 163 | V163 | f_0x08:u32, f_0x0c:u32, f_0x10:camera, f_0x4c:obj, f_0x48:ref, f_0x50:obj, f_0x54:bool, … +1 | 757 | 63 | Camera-controller operation on an entity/camera | ❓ refs CameraController |
| 220 | SetEntityTransformStatement | f_0x08:entity, f_0x34:point, f_0x48:entity, f_0x74:obj, f_0x78:u32, f_0x7c:u32, f_0x80:label, … +2 | 705 | 76 | Set an entity transform from a point/entity (+helper label) | 🟡 |
| 448 | V448 | f_0x0c:entity, f_0x38:obj, f_0x68:point, f_0x3c:entity, f_0x7c:label, f_0x04:u32, f_0x94:obj, … +1 | 679 | 51 | Entity/point operation with helper label | ❓ |
| 413 | V413 | f_1:obj, f_0x0c:obj, f_0x10:obj, f_0x14:obj, f_0x18:label, f_0x30:label, f_0x48:u32 | 678 | 55 | Rel* operation with two labels | ❓ |
| 423 | V423 | f_1:u32, f_2:obj, f_3:u8, f_0x0c:obj, f_0x14:obj | 659 | 66 | Unknown | ❓ |
| 353 | V353 | f_0x08:u32, f_0x0c:u32, v166:camera, f_0x48:ref, f_0x4c:obj, onComplete:script, f_0x6c:obj, … +1 | 600 | 50 | Sprite/material animation with `onComplete` script | ❓ |
| 466 | V466 | f_1:uiTarget, expressions:stmts | 526 | 39 | UI target + statement list (for each UI element?) | ❓ |
| 268 | V268 | f_0x08:entity, f_0x34:enumEntry/ref, f_0x50:enumEntry/ref, f_0x4c:u32, f_0x68_1:anim, f_0x68_2:ref | 345 | 29 | Player-controller/animation operation | ❓ |
| 337 | SetUIFocusStatement | target:uiTarget | 343 | 33 | Move UI focus to a dialog element | 🟡 |
| 285 | V285 | f_1:entity, f_0x34:obj, f_0x39:bool, f_0x38:bool | 303 | 24 | Entity flag set from Const* entry | ❓ |
| 509 | SelectContextObjectStatement | slot:u32, objects:obj[], index:obj | 286 | 24 | Select an object from a list into a context slot by index | 🟡 |
| 283 | V283 | v301:entity, v421:enumEntry, old:ref, old2:u32, f_0x4c:u32, f_0x50:ref, v301_2:entity, … +1 | 279 | 21 | PCtrl* component operation | ❓ |
| 472 | V472 | f_0x00:entity, f_0x34:u32, f_0x38:u32, f_0x3c_1:point, f_0x3c_2:ref, f_0x50:entity, f_0x7c:label | 254 | 24 | Entity/point operation with helper label | ❓ |
| 427 | V427 | f_0x08:camera, f_0x40:bool, f_0x48:u32, f_0x4c:ref, f_0x44:u32 | 246 | 26 | Apply a CameraEffect to a camera | ❓ |
| 478 | V478 | f_1:uiTarget, f_0x14:obj, f_0x18:u32 | 225 | 13 | UI target operation | ❓ |
| 165 | CreateTemporaryBillboardStatement | f_0x0c:bool, duration:f32, material:ref, f_0x14:obj | 224 | 16 | Spawn a temporary billboard sprite for `duration` seconds | 🟡 |
| 514 | V514 | f_1:uiTarget, v421:enumEntry, f_0x2c:obj[], f_0x34:obj, f_0x38:bool | 210 | 17 | UI target + enum entry (Msg*/System*) + args: send message to UI? | ❓ |
| 169 | ViewportStatement | operation:u32, layout:u32, f_0x10:ref, viewportIndex:obj | 209 | 18 | Viewport layout operation | 🟡 |
| 251 | V251 | f_0x08:trigger, f_0x1c:bool | 204 | 20 | Enable/disable a Trigger | ❓ refs Trigger |
| 416 | V416 | v166:camera, f_0x40:obj, f_0x44:obj | 200 | 15 | Camera operation with constants | ❓ |
| 168 | SetPlayerViewportStatement | player:obj, viewport:obj | 198 | 13 | Assign a player to a split-screen viewport | 🟡 |
| 178 | ChangeMapStatement | f_0x0c:bool, f_0x10:obj, f_0x14:u32, f_0x18:u32, f_0x1c:bool, f_0x20:obj, f_0x24:id, … +2 | 186 | 18 | Load another map | 🟡 |
| 424 | NetworkClientStatement | operation:u32, argument:obj | 170 | 15 | Network client operation (op, argument) | 🟡 |
| 384 | AssignStringStatement | destination:obj, source:obj | 156 | 11 | Assign a string | 🟡 |
| 362 | V362 | f_1:u32, f_2:ref, f_0x10:u32, f_0x14:u32, f_0x18:obj, f_0x1c:obj[] | 138 | 12 | Unknown (ref + list) | ❓ |
| 125 | FollowPathStatement | flags:u32, entity:entity, path:path, f_0x4c:obj, f_0x50:obj, onCompleted:script | 122 | 9 | Make an entity follow a path; run `onCompleted` at the end | 🟡 |
| 479 | XboxSystemStatement | operation:u32 | 116 | 8 | System operation; name says Xbox but 116 uses on PS2 | ❓ name doubtful |
| 516 | V516 | f_1:ref+u32, f_2:uiTarget, f_0x14:u32 | 113 | 11 | UI operation with ref+u32 | ❓ |
| 385 | V385 | f_0x08:u32, f_0x0c:obj, f_0x10:obj, f_0x14:obj | 108 | 9 | Unknown (dialog) | ❓ |
| 328 | V328 | f_0x08:entity, f_0x4c:obj, f_0x34:enumEntry/ref, f_0x50:u32 | 89 | 7 | Attack* enum operation on an entity | ❓ |
| 167 | AssignPlayerEntityStatement | old:u32, player:obj, target:entity | 88 | 6 | Bind a player slot to an entity | 🟡 |
| 336 | V336 | f_1:camera, f_0x40:u32, f_0x44:u32, f_0x48:u32, f_0x4c:bool | 86 | 0 | Camera operation (three u32 + bool) | ❓ |
| 394 | V394 | f_1:u32, f_0x0c:obj | 84 | 4 | Gs* game-state operation | ❓ |
| 434 | V434 | f_1:camera, f_0x40:bool, f_0x44:u32 | 84 | 6 | Camera operation (bool + u32) | ❓ |
| 331 | V331 | old:u8, f_1:u32, f_0x10:obj, f_0x0c:obj | 45 | 2 | Unknown (two values) | ❓ |
| 372 | EditUITextStatement | target:uiTarget, operation:u32, text:obj, position:obj | 42 | 2 | Edit UI text (operation, text, position) | 🟡 |
| 329 | V329 | f_1:entity, f_0x4c:obj, v421:enumEntry, old:ref, old2:u32, f_0x50:u32, v421_2:enumEntry, … +2 | 32 | 2 | PCtrl*/Attack* component operation | ❓ |
| 338 | V338 | f_1:label, f_0x20:bool | 31 | 0 | Unknown (label + bool) | ❓ |
| 334 | V334 | f_1:u32, f_0xc:camera | 28 | 0 | Camera operation (mode) | ❓ |
| 223 | SetTimerEnabledStatement | timer:timer, enabled:bool, resetElapsedTime:bool | 5 | 0 | Enable/disable a Timer, optionally reset elapsed time | 🟡 |
| 205 | V205 | v301:entity, f_0x34:obj, f_0x38:u32, f_0x3c:bool, f_0x40:obj | 4 | 1 | Entity operation (selector, value, bool) | ❓ (4 uses) |
| 113 | V113 ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |
| 180 | V180 ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |
| 332 | V332 ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |
| 343 | V343 | f_0x0c:entity, f_0x08:u32, f_0x38:u32, f_0x3c:u32, f_0x40:u32, old:ref, f_0x44:template, … +1 | 0 | 0 | (not seen on PS2) | ❓ |
| 393 | V393 | v301:entity, f_0x34:vec3, f_0x40:u32, f_0x44:bool | 0 | 0 | (not seen on PS2) | ❓ |
| 408 | V408 ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |
| 445 | V445 ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |
| 447 | SetCameraColorStatement ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |
| 450 | V450 ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |
| 454 | V454 | f_1:u32, f_0x0c:obj, f_0x10:bool | 0 | 0 | (not seen on PS2) | ❓ |
| 459 | V459 | f_1:u32, f_0x0c:u32, f_0x10:obj, f_0x14:bool | 0 | 0 | (not seen on PS2) | ❓ |
| 475 | V475 ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |
| 476 | V476 ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |
| 499 | V499 | f_1:u32, f_0x0c:u32, f_0x10:obj | 0 | 0 | (not seen on PS2) | ❓ |
| 507 | V507 ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |

### Expressions (78 classes)

| id | Class (vicious name) | Own fields | Uses, all PS2 | Uses, focus | Meaning (ours) | Confidence |
|---:|---|---|---:|---:|---|---|
| 114 | FloatLiteralExpression | value:f32 | 98,434 | 7,685 | Float literal | ✅ values 0,1,2,-1,… dominate |
| 159 | ExecutionContextValueExpression | source:u32, index:u32 | 89,298 | 6,872 | Read a value from the execution context: (source, index); source 0/1/2 = probably message argument / local / result | 🟡 |
| 111 | BinaryOperationExpression | operation:u32, operands:obj[2] | 68,989 | 5,102 | Binary operator on two operands (op codes 0–16) | 🟡 op codes ❓ |
| 210 | V210 | f_0x08:entity, f_1:enumEntry/ref | 39,972 | 3,120 | **Read an entity variable** (Var* enum entry) of the selected entity | 🟡 strong (Var* 36,293/39,459) |
| 341 | AssetElementIndexExpression | element:enumEntry | 35,281 | 2,719 | Enum-entry literal (value of an Enum*/Msg*/Mem* entry) | 🟡 |
| 110 | UnaryOperationExpression | operation:u32, operand:obj | 19,740 | 1,487 | Unary operator (op 0 in 86%: probably negate or not) | 🟡 op codes ❓ |
| 358 | V358 | f_1:ref | 16,549 | 1,131 | Read an ExternalVariable (#357), e.g. GameMode | ✅ ref→ExternalVariable 16,549/16,549 |
| 228 | EntityExistsExpression | entity:entity, template:template, matchTemplate:bool | 11,994 | 935 | Does the entity (of template) exist | 🟡 |
| 212 | V212 | f_0x08:entity, f_1:enumEntry/ref | 8,593 | 742 | **Read an entity constant** (Const* entry) | 🟡 strong (Const* 8,168/8,282) |
| 436 | V436 | f_0x04:obj, f_0x08:u32, f_0x0c:enumEntry, f_0x24:u32 | 7,588 | 433 | Read a game-state variable (Gs* entry) | 🟡 data: Gs* 6,270/7,003 |
| 307 | V307 | f_0x04:obj, f_1:enumEntry/ref | 7,069 | 540 | Is entity in category (Cat* entry) | 🟡 data: Cat* 6,503/7,040 |
| 444 | ObjectExistsExpression | object:obj | 6,429 | 493 | Object exists (is the Rel* slot set) | 🟡 |
| 158 | V158 | f_1:ref | 6,370 | 473 | Read a Constant (#123) | ✅ ref→Constant 6,370/6,370 |
| 272 | V272 | f_0x08:entity, f_1:enumEntry/ref | 5,828 | 445 | Timer value (Time* entry) of an entity | 🟡 data: Time* 5,736/5,820 |
| 152 | V152 | f_1:uiTarget, f_2:u32 | 5,423 | 461 | Dialog/UI state query (often a switch target) | ❓ |
| 321 | ObjectEqualsExpression | left:obj, right:obj | 5,333 | 379 | Object equality | 🟡 |
| 279 | V279 | — | 4,601 | 379 | No fields: an implicit value (self / current entity / time?) | ❓ |
| 148 | V148 | f_1:uiTarget | 3,534 | 282 | Dialog/UI state query | ❓ |
| 282 | RandomRangeExpression | mode:u32, integerResult:bool, minimumOrCenter:obj, maximumOrRadius:obj | 3,501 | 257 | Random number in range | 🟡 |
| 203 | EntityMotionPropertyExpression | entity:entity, property:u32 | 3,472 | 279 | Entity motion property | 🟡 |
| 112 | TernaryOperationExpression | exclusiveBounds:u32, operation:u32, operands:obj[] | 3,054 | 242 | Ternary/range operation (bounds flag, op, operand list) | ❓ |
| 240 | EntityAimAngleExpression | f_0x04:u32, f_0x08:entity, f_0x34:entity, f_0x74:label, f_0x8c:label, f_0xa4:obj, f_0xac:u32, … +10 | 2,768 | 221 | Aim angle between entity and target | 🟡 |
| 137 | ControllerInputExpression | property:u32, inputIndex:u32, entity:entity | 2,613 | 196 | Controller input value for a player/entity | 🟡 |
| 320 | V320 | f_0x08:obj, f_1:enumEntry/ref, f_0x20:bool | 2,215 | 176 | Relation-slot test (Rel*) | ❓ |
| 211 | V211 | f_0x04:entity, f_1:ref | 2,142 | 168 | Entity variable query, other form (Var*) | ❓ |
| 229 | EntityDistanceExpression | sourceEntity:entity, targetEntity:entity, targetPoint:point, targetMode:u32, sourceHelper:label, targetHelper:label, squared:bool, … +4 | 2,105 | 161 | Distance between entities/points | 🟡 |
| 151 | V151 | f_1:uiTarget, f_2:u32 | 1,835 | 128 | Dialog/UI state query | ❓ |
| 147 | V147 | f_1:uiTarget | 1,609 | 112 | Dialog/UI state query | ❓ |
| 330 | MultiplayerStatusExpression | property:u32, f_2:obj | 1,485 | 106 | Multiplayer status value | 🟡 |
| 449 | FileSystemStatusExpression | property:u32, expectedErrorCode:u32 | 1,116 | 111 | File system status | 🟡 |
| 202 | V202 | f_1:u32 | 1,008 | 72 | Game-state value by number | ❓ |
| 344 | V344 | f_0x04:entity | 918 | 83 | Entity property | ❓ |
| 213 | V213 | f_0x04:entity, f_1:ref | 829 | 62 | Entity constant query, other form (Const*) | ❓ |
| 149 | V149 | f_1:uiTarget | 676 | 51 | Dialog/UI state query | ❓ |
| 207 | CameraPropertyExpression | camera:camera, property:u32 | 644 | 46 | Camera property | 🟡 |
| 271 | V271 | f_0x04:entity, f_0x30:enumEntry/ref, f_0x48:u32 | 634 | 60 | PCtrl* component value | ❓ |
| 250 | NavigationHeadingExpression | f_1:u32, f_2:entity, f_0x30:obj, f_0x34:u32, f_0x38:u32, f_0x3c:label, f_0x54:u32 | 556 | 46 | Navigation heading | 🟡 |
| 273 | V273 | f_0x08:entity, f_1:enumEntry/ref | 551 | 43 | Animation query (Anim* entry) | 🟡 |
| 146 | V146 | f_1:uiTarget, f_0x10:u32, f_0x14:u8, f_0x18:u32 | 535 | 31 | Dialog/UI state query | ❓ |
| 145 | V145 | f_1:uiTarget, f_0x10:u32, f_0x14:u8, f_0x18:u32 | 452 | 30 | Dialog/UI state query | ❓ |
| 438 | V438 | f_1:u32, f_2:obj, f_3:obj | 415 | 32 | Unknown (u32, value, object) | ❓ |
| 480 | V480 | f_0x04:f32 | 412 | 31 | f32 value | ❓ |
| 389 | NetworkClientPropertyExpression | property:f32 | 411 | 21 | Network client property | 🟡 |
| 456 | EntityComponentExistsExpression | entity:entity, component:enumEntry, requireEnabled:bool | 396 | 34 | Entity has component (PCtrl* entry) | 🟡 |
| 157 | V157 | f_1:ref | 373 | 44 | Read a global Variable (#121) | ✅ ref→Variable 370/372 |
| 400 | MapLoadStatusExpression | property:u32, f_2:bool | 342 | 26 | Map load status | 🟡 |
| 497 | V497 | f_1:uiTarget, f_0x10:u32, f_0x14:obj | 320 | 17 | UI target value | ❓ |
| 492 | StringEqualsExpression | left:obj, right:obj, ignoreCase:bool | 215 | 19 | String equality | 🟡 |
| 274 | V274 | f_0x08:entity, f_1:enumEntry/ref | 198 | 14 | Timer query (Time*) | ❓ |
| 457 | EntityStateIndexExpression | entity:entity, property:u32 | 197 | 16 | Entity state index | 🟡 |
| 311 | EntityVelocityExpression | entity:entity, property:u32 | 195 | 15 | Entity velocity | 🟡 |
| 216 | V216 | f_1:entity, f_2:u32 | 176 | 13 | Entity property (u32) | ❓ |
| 415 | EntityAuthorityExpression | entity:entity | 163 | 15 | Entity has network authority | 🟡 |
| 164 | V164 | f_1:entity, f_0x30:u32, f_0x34:u32 | 159 | 18 | Entity property (two u32) | ❓ |
| 378 | V378 | f_1:uiTarget, f_0x10:u32, f_0x14:u32 | 158 | 9 | UI target value | ❓ |
| 364 | EntityScreenProjectionExpression | entity:entity, label:label, camera:camera, property:u32, predictionMode:u32, predictionSpeed:obj, f_0x8c:obj, … +1 | 124 | 8 | Project entity to screen | 🟡 |
| 414 | NetworkSessionModeMatchExpression | expectedMode:f32 | 105 | 5 | Network session mode equals | 🟡 |
| 150 | V150 | f_1:ref+u32, f_2:uiTarget, f_0x10:u32, f_0x14:u32, f_0x18:bool, f_0x1c:u32 | 72 | 0 | Dialog/UI state query | ❓ |
| 386 | NetworkModeMatchExpression | expectedMode:f32 | 58 | 3 | Network mode equals | 🟡 |
| 390 | V390 | f_1:u32, f_0x0c:u32, f_2:obj | 28 | 1 | Unknown | ❓ |
| 208 | ClampContextValueExpression | bounds:obj[2] | 19 | 2 | Clamp context value to bounds | 🟡 |
| 144 | V144 | f_1:uiTarget | 16 | 2 | Dialog/UI state query | ❓ |
| 482 | V482 | v301:entity, f_0x30:obj | 6 | 0 | Entity + value | ❓ |
| 477 | MapNetworkFlagsExpression | — | 3 | 1 | Map network flags | 🟡 |
| 455 | CameraExistsExpression | camera:camera | 2 | 0 | ❓ | ❓ |
| 510 | VoiceStatusExpression | property:u32 | 1 | 1 | Voice status | 🟡 |
| 109 | GameStateExpression | — | 0 | 0 | (not seen on PS2) | ❓ |
| 277 | V277 ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |
| 417 | V417 ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |
| 432 | V432 ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |
| 439 | V439 ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |
| 440 | V440 ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |
| 451 | BallisticAimExpression ⚠todo | — | 0 | 0 | (not seen on PS2) | ❓ |
| 453 | V453 | flags:u32, f_1:u32, self:obj | 0 | 0 | (not seen on PS2) | ❓ |
| 458 | V458 | flags:u32, f_1:u32, self:obj | 0 | 0 | (not seen on PS2) | ❓ |
| 484 | NetworkClientFlagExpression | property:u32 | 0 | 0 | (not seen on PS2) | ❓ |
| 496 | NetworkStateMatchExpression | property:u32, expectedValue:u32 | 0 | 0 | (not seen on PS2) | ❓ |
| 500 | V500 | f_1:u32, f_0x00:obj | 0 | 0 | (not seen on PS2) | ❓ |

## 4. What exists in `game/src/` today
| File | What it is | Relevance |
|---|---|---|
| `script.rs` | **Test-input scripting**, not game logic: `.script` files with `at <t> place/move/press/expect …` on a fixed 1/60 s step and a fixed seed | The VM must not reuse the name `script`. We will extend this test harness (§7) |
| `rules.rs` | Hand-written classic-mode rules (rooms, furniture, items, traps, doors, match state) | Stays for classic mode; arena mode moves to data-driven behaviour |
| `ai.rs` | Hand-written AI `Brain` for classic mode | Same. The data has `HL_AISpyBase`/`LL_*` state machines for later |
| `main.rs` | Bevy app: classic mode systems, `ScriptRun` resource, HUD, sounds, screenshots; `SVS_MAP` switches to `arena::plugin` | Where the VM plugin is registered |
| `arena.rs` | Real levels: loads the map, places entities and templates, walking spies, `SVS_ARENA_TEST` walk test | First consumer of the VM (doors) |
| `model.rs` | Map readers: render meshes, materials, `DynamicMesh` templates, entities (`read_ref`, `chunk_at`, `children`) | Reuse its chunk helpers. The VM decoder generalises `read_ref` |
| `level.rs`, `gamedata.rs` | Collision, PAK/ISO access, textures, sounds | Collision toggling for doors |

## 5. Interpreter design

### 5.1 Module layout (new, under `game/src/vm/`)
| Module | Job |
|---|---|
| `vm/stream.rs` | Byte reader for the container: chunk begin/end (always **seek to `end`**, so unknown fields are skipped safely), `AssetFromType`, `AssetReference` (with id → object table), `Label`/`LabelID`, versioned field helpers |
| `vm/decode.rs` | `class id → decoder` for the ~140 script classes plus the owners (`Script`, `StateMachine`, `V181`, `V209`, `V218`, `V200`, `V258`, `V174`, `Enumeration`, `Function`, `Variable`, `Constant`, `ExternalVariable`, `EntitySelector`, `V421`). Unknown ids decode to `Node::Unknown { class, version }` and are skipped via `end`. Written by us from the field tables in these notes, not generated from vicious |
| `vm/ast.rs` | `Stmt` and `Expr` enums (one variant per implemented class, `Unknown` otherwise), `Script = Vec<Stmt>`, `Machine { states, functions, handlers }`, `Handler { event: EnumRef, script, … }` |
| `vm/symbols.rs` | Enumerations (name ↔ entry id), global `Variable`/`Constant`/`ExternalVariable` tables, `Function` table, object ids → decoded objects |
| `vm/value.rs` | `Value` enum: `Num(f32)`, `Obj(ObjRef)`, `Str(String)`, `Enum(EnumRef)`, `None`; truthy = non-zero |
| `vm/exec.rs` | Tree-walking interpreter: `Frame { self_entity, sender, args, locals, result }`, `exec(&Script)`, `eval(&Expr)`, control flow (`Return`/`Break` as results), **step budget** per call (infinite-loop guard) |
| `vm/machine.rs` | Per-entity machine instance: current state, entity vars (`Var*` → f32), consts (`Const*`), relations (`Rel*` → ObjRef), timers. State change = run `State.Exit` of the old state, switch, run `State.Enter` of the new one. Message dispatch = current state's `V209` handlers first, then the machine's `V218` handlers ❓ (order to confirm with the friend) |
| `vm/host.rs` | `trait Host`: everything that touches the world (find entity, transform, spawn, play animation/sound, collision on/off, camera, UI). Arena mode implements it; unit tests use a mock host that records calls |
| `vm/plugin.rs` | Bevy glue (§5.3) |
| `vm/trace.rs` | Deterministic event trace (`t, entity, state change / message / host call`) for tests |

### 5.2 Decoding route
At runtime, read the map bytes with our own Rust decoder (the game already reads the disc directly; no vicious
C at runtime). For development, use ps2-decode's JSON (`work/ps2-decode/json/`, when it exists) as an oracle:
a `cargo test` compares our decoded node counts and fields per class with the JSON for the focus maps.

### 5.3 Hooking into Bevy
- `Resource ScriptVm { symbols, machines: Vec<Machine>, instances: HashMap<Entity, Instance>, queue: VecDeque<Msg>, trace }`.
- `Component Behaviour { instance }` on every spawned map entity whose template has a `V258` slot.
- Systems in `FixedUpdate` at 60 Hz (the same step `script.rs` uses), chained:
  1. `vm_events`: turn world events into messages (trigger enter/exit → `Trigger` scripts; `AnimationCompleted`,
     `PathCompleted`, `OnCollided` → `SystemEvents`);
  2. `vm_dispatch`: drain the message queue (bounded per frame);
  3. `vm_update`: run `State.Update` handlers and timers (`State.Timer`, `Timer` objects);
  4. `vm_apply`: apply recorded host commands (transforms, collision toggles, animations, sounds) to ECS.
- Map start: build instances, set initial vars from `V174`, run `Map.onMapStarted` and each machine's initial
  state `Enter`.
- Determinism: fixed step, seeded RNG for `RandomRangeExpression`, no wall-clock time. That keeps traces
  replayable for tests and later for netcode lockstep.

### 5.4 Implementation order
Driven by focus-map usage (62,455 nodes). Cumulative coverage is the share of focus nodes whose class is
implemented after each phase.

| Phase | Classes | Coverage after |
|---|---|---|
| A. Decode only | all classes in §3 parse to their chunk `end`; unknown → skipped | 0% executed; decode coverage target 100% of focus nodes |
| B. Core language | statements 120, 118, 119, 215, 214, 245, 465, 420; expressions 114, 111, 110, 112, 159, 341, 158, 157, 358, 282 | **66%** |
| C. State machines + messages | `V181`/`V209`/`V218`/`V200` runtime; 192 (send), 194 (change state), 201 (call local fn), 115 (call fn); 210/212/211/213 (entity var/const), 436 (Gs), 272/274 (Time), 202, 279 | **82%** |
| D. Relations and context | 312, 246, 509; 444, 321, 320, 307, 228 | **89%** |
| E. World effects | 193 (anim), 335/249 (sound), 220 (transform), 224 (spawn), 308 (material), 251 (trigger), 223 (timer), 125 (path), 165; queries 203, 229, 240, 311, 273, 457, 456, 250, 137 | **92%** |
| F. The rest | UI/dialog family (138, 253, 144–152, 367, 337, …), cameras (163, 326, 334, 336, 416, 427, 434), PCtrl family (266–268, 271, 283), network (424, 330, 386, 389, 414, 415), 345/346/310 | remaining 8%; stub with a logged no-op first |

Operator codes (`111` 0–16, `110` 0–19, `120` 0–4) and context sources (`159` 0–2) are the main open
semantics. Start with the obvious guesses behind a table that is easy to fix (`+ − × ÷`, comparisons, logic),
and confirm each against the friend's Ghidra evaluator (vicious-review §8). Write down only the meaning, in
our own words.

## 6. First milestone: a door opens by its own script
Target: `Door_*` entities on **Mansion_Story** run the data's `Door` state machine.

Scope ✅ (`doorscope.py`): the `Door` machine has 5 states, 7 local functions and 7 machine-level handlers, and
**336 script nodes of 38 classes**. In order of count: If 52, FloatLiteral 31, V210 26, AssignValue 26,
Binary 22, Return 20, V201 16, V194 16, AssetElementIndex 15, V192 12, ExecutionContextValue 10,
AssignFileReference 9, V358 8, V320 7, V279 6, then single-digit counts (Unary, EntityDistance, V211, V273,
V310, EntityExists, ObjectExists, ObjectEquals, V212, V193, V213, SpawnEntity, V345, V346, PlaySound, Case,
Break, V307, V195, EntityAimAngle, SetContextObject, V308, Switch). Phases B–C cover most of it. Spawn (trap),
sound and material can be host no-ops at first.

Steps:
1. Decode every `StateMachine` in Mansion_Story; `cargo test`: Door has states
   `Closed/Closing/Open/Opening/Locked`, 7 functions, 7 machine handlers, 336 nodes.
2. Instantiate the machine for each `Door_*` entity (via its template's `V258`), with initial vars from `V174`.
3. Mock-host unit test: send `MsgDoor.Open` (and `MsgGame.Interact`) to a closed door. Expect the trace to reach
   `Opening`, a host call to play an `AnimDoor` animation, and after a synthetic `SystemEvents.AnimationCompleted`
   the state `Open`. Repeat for `Close` and `Lock`/`Unlock`.
4. Arena host: while a door is in `Opening`/`Open`, remove its collision (replaces today's invisible walls from
   `solid_class("Door_")`) and rotate the door model by the clip (or a simple swing until skinning lands).
5. Later, when the spy's own `HL_SpyBase`/`LL_Spy` machines run: the spy pressing interact sends the message
   itself. Until then, the test script injects it.

Exit criterion: the step 3 trace test passes headless, and a `.script` test (§7) opens a Mansion_Story door and
walks through it.

## 7. Test approach
- **Unit tests (`cargo test`, headless, run now)**: decode coverage per focus map, i.e. every node parses to its
  chunk end, the count of `Unknown` nodes is tracked over time, and per-class counts match §3. Interpreter tests
  use hand-built ASTs. Mock-host trace tests replay a message list and compare the event trace with an expected
  trace file (`game/tests/vm/*.trace`).
- **Replayable `.script` tests** (existing harness, `script.rs`): add commands
  - `at <t> send <entity-name> <Enum.Entry>` (inject a message),
  - `at <t> expect <entity-name> state <StateName>`,
  - `at <t> expect <entity-name> var <Var*.Entry> <value>`,
  - `at <t> expect trace <file>` (compare trace so far).
  These run under arena mode with a fixed step and seed, per the project's "inputs + expected end state" rule.
- **Not now**: `tests/run_all.sh` and `ref_compare.py` open windows. Run them only when Simon is not playing.
  A headless arena mode (Bevy `MinimalPlugins`, no window) would let the `.script` VM tests run under
  `cargo test`. Worth adding with phase C.

## 8. Open questions
| Question | Who / how |
|---|---|
| Operator tables for 110/111/120, context sources for 159 | friend's Ghidra (evaluator `switch`) |
| Message dispatch order (state handlers vs machine handlers vs `OnEnumerationEvent`), `Enter`/`Exit` order on state change | friend's Ghidra; our trace tests then lock it in |
| Meaning of `V217` second ref + `u32` (a declarative transition target?) and of `V255` (guard or parameter binding?) | data: compare against handler scripts; friend |
| `V279` (no fields, 7,827 uses) | evaluator code; likely "self" or "time" |
| `EntitySelector` flags (`FN_0x21c40` three `u8`) | data + friend |
| How the spy's input reaches its state machine (`PCtrl*`, `MsgPlayerHigh`) | after milestone 1 |
