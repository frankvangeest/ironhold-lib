# Authoring tools: check, watch and inspect your project

> **Audience:** game designers and anyone writing RON. The `ironhold` command-line tool is **optional** — everything
> in the engine works without it — but it catches most authoring mistakes before you ever start the game.

The `ironhold` tool reads your project's RON files and assets **without starting the
engine**. Elsewhere in these docs it also appears as `ironhold_cli validate …`; it is the same tool.

> **If you only have the web build** (no tool): you lose the early checks, not the engine. The same mistakes show up
> later as warnings in the browser's DevTools console (F12 → Console), and a RON typo shows up as an asset-load error
> there. If the loading screen never finishes, open the console: a broken `.project.ron` stops loading entirely.
> Everything the tool checks is also described in [20_data_formats.md](20_data_formats.md).

## 1. Getting the tool

Today you build it once from the repository: `cargo build -p ironhold_cli --release`. That produces a single
`ironhold` program (`ironhold.exe` on Windows) in your Cargo output folder (`target/release` unless your machine
redirects it) that needs no install, so an engineer can build it once and hand you the file. You can also run it
without copying anything: `cargo run -p ironhold_cli -- <args>`. A prebuilt download on a GitHub release is planned;
this section will link to it when it exists.

## 2. The edit–check loop

1. Start `ironhold watch <project_dir>` in a terminal and leave it running. Every time you save a `.ron` file in the
   project it re-checks and prints one line: `OK`, or the problems with the file, line and column.
2. Before you commit or share a project, run `ironhold validate <project_dir>` once; add `--strict` to also catch
   dead data (see §4).
3. Exit codes (useful in scripts): `0` all valid · `1` errors, or strict warnings when `--strict` · `2` tool/IO error.

```bash
ironhold watch    assets/projects/quick_scene/
ironhold validate assets/projects/particles_demo/
ironhold validate --strict assets/projects/particles_demo/
```

Sample `watch` output (each save prints the changed file and a result):

```
[14:24:15] scenes\main.scene.ron
           →  ERROR (1 issue)
             scenes/main.scene.ron: line 12, col 5: unknown field `directioonal`

[14:24:32] scenes\main.scene.ron
           →  OK (6 files)
```

(The tool prints your operating system's path separator in its own output. In RON files always write `/`.)

## 3. Reading a report

- **Fix the first parse error first.** If a file does not parse, the checks that depend on it (for example the
  button-wiring checks when `state_machine.ron` is broken) are skipped rather than guessed, so later problems only
  appear once the earlier one is fixed.
- **Per-file errors** (parse errors with line and column) come first; **Cross-file checks** are mistakes that only
  show up when files are compared (a key that does not exist in `assets.ron`, a button nothing listens to).
- **Strict checks** (`--strict`) are advisory "dead data" and "probably not what you meant" findings; they also make
  the exit code `1`.
- **Finding a problem's code.** The plain output prints the message but **not** the short diagnostic code. Add
  `--json` (before the command name: `ironhold --json validate …`) to see each problem's `type`, e.g.
  `unreachable_trigger`. The tables below list both the message wording and the code so you can look either up.
  Parse errors report the type `parse_error`.

## 4. What `validate` catches

**Always checked** (errors). "Fix" says what to change; the matching format is in [20_data_formats.md](20_data_formats.md).

### Typos and missing keys
| What you see / what is wrong | Code | What breaks if ignored | Fix |
|---|---|---|---|
| a RON parse error (line and column, e.g. "unknown field") | `parse_error` | the file is ignored or the asset fails to load | fix the spelling/structure at that line; struct fields are strict |
| an effect/decal/audio/prefab/modifier key used in an action or scene that is not defined in `assets.ron` / `prefabs.ron` / `stats.ron` | `missing_reference` | the action does nothing or the entity does not spawn | add the key to the catalog or fix the typo |
| a prefab's `behavior` / `dialogue` / `animation_policy` file path that does not exist | `missing_file` | a character with a missing animation policy is **permanently invisible** in the web build | fix the path |
| a `Spawn(...)` action's `spawn_point` is not in any scene's `spawn_points` | `missing_reference` | the entity silently appears at the world origin | add the spawn point or fix the name (a `{self}`/`{target}` template is skipped) |
| a merchant's `currency_stat`, a `stock[].item_key`, an `inventory.initial_items[].item_key`, an `AddItem`/`RemoveItem`/`TransferItem`/`BuyItem` item key, an `ItemDef.currency_stat`, or an `interactable.requires_item` that does not exist | `missing_reference` | the shop/item does not work; a `requires_item` typo **permanently blocks every player** | fix the key |
| a dialogue `jump_to` that names no node (and is not `"__end__"`) | `missing_reference` | the dialogue panel silently closes mid-conversation | fix the node id |
| a dialogue condition `stat_key` that is not in `stats.ron` | `missing_reference` | the choice is hidden forever, with no message | fix the stat key |
| two dialogue nodes with the same `id` in one file | `duplicate_node_id` | every later duplicate is unreachable | make ids unique |
| an `ItemDef.icon_sheet`, an action-bar `icon`/`icon_sheet`, an `IconButton` `icon_on`/`icon_off`, a world stat bar icon/texture, or a `target_indicator.texture` (looked up in `decals`, not `textures`) that is not in the matching `assets.ron` section | `missing_catalog_key` / `missing_reference` | a blank image with no warning | add or fix the key |

### Files and paths
| What you see | Code | What breaks if ignored | Fix |
|---|---|---|---|
| a scene/dialogue/behavior/catalog/`state_machine_path` path that does not exist (including `initial_scene` and every `LoadScene`/`LoadSceneOverlay`/`PreloadScene`/`ToggleOverlay` target) | `missing_file` | the scene never loads | fix the path; scenes outside `scenes/` are parsed and checked too |
| a path that exists but with the wrong **case** or a `\` separator | `path_case_mismatch` | works on Windows, **404s in the browser** | write every path with forward slashes and the file's exact on-disk casing |
| an `assets.ron` model/texture/audio/decal path, a material's texture/shader/splatmap path, a `global_environment` path, or a scene terrain path whose file is missing or mis-cased | `missing_file` / `path_case_mismatch` | 404 in the browser | fix the path (these are relative to the `assets/` folder) |
| an absolute path (pasted from a file browser) in any of those fields | `absolute_asset_path` | resolves against your machine only | use a path relative to `assets/` |
| a catalog path configured in the `.project.ron` that does not exist | `missing_file` | that catalog does not load | fix the field |

### Logic wiring
| What you see | Code | What breaks if ignored | Fix |
|---|---|---|---|
| a `Button`, `IconButton`, key binding or gamepad binding whose `ui.button_pressed:<trigger>` is not handled by any binding in `state_machine.ron` or a `behaviors/*.behavior.ron` file (also the built-in panel triggers such as `close_inventory`, `take_all_from_container`, `buy_item:<key>` when that panel is in a scene) | `unreachable_trigger` | "I clicked the button and nothing happened" | add a binding for the trigger, or remove the button |
| a state machine / behavior / dialogue file that fails to parse | `parse_error` | the button-wiring checks are skipped for the whole project | fix that file's parse error first |

### Scenes and UI layout
| What you see | Code | What breaks if ignored | Fix |
|---|---|---|---|
| a scene with a wrong `schema_version`, an empty or duplicate entity / world-label / UI id (a `Group` may have no id; UI ids must be unique across nested groups) | `invalid_scene` | unpredictable results; only the first problem per scene is reported, so fix and re-run | make ids unique and non-empty |
| a `Group` with a negative or non-finite `gap`/`padding`/`width`/`height` | `invalid_group_value` | the layout is wrong | use numbers >= 0 |
| a `Group` nested deeper than 16 levels | `ui_depth_exceeded` | the deeper children are **never spawned** | flatten the nesting |
| a `Label`/`Button` with `font_size` <= 0 | `invalid_font_size` | renders nothing | use a positive size |
| a `label_depth_scale.min_scale` outside `[0.0, 1.0]` | `label_depth_scale_min_scale_out_of_range` | labels scale incorrectly | use a value in range |

### Cameras and input
| What you see | Code | What breaks if ignored | Fix |
|---|---|---|---|
| a `camera_modes:` preset named the reserved key `"default"` | `reserved_camera_mode_key` | `SetCameraMode(mode: "default")` always restores the camera's own starting mode, never your preset | rename the preset |
| a `Party(...)` entry in the `camera_modes:` registry | `unsupported_registry_camera_mode` | a `SetCameraMode` cannot reach it | use `Orbit(...)` or another single-camera mode |
| a `Fixed.look_at_entity` that does not exist, or a `SetCameraMode` whose `mode` is in no scene's registry | `missing_reference` | the camera mode never applies | fix the entity id or the registry |
| `split`/`party` written **inside** a `camera_mode: Orbit(...)` instead of next to it under the prefab's `components:` | `camera_mode_nested_split_party` | silently ignored | move them next to `camera_mode` (in a `camera_modes:` registry entry, delete them) |
| `Party(...)` as a player prefab's own `camera_mode`, or a `flycam`-tagged prefab whose `camera_mode` is not `Flycam(...)` | `unsupported_prefab_camera_mode` | the runtime silently falls back | use `Orbit(...)` / `Flycam(...)` |
| an unrecognised `orbit_button` / `character_rotate_button` / `look_button` value | `invalid_binding` | falls back to `"Either"` | use a supported value (see [20_data_formats.md](20_data_formats.md)) |
| an unrecognised flycam movement key (`forward`, `backward`, `left`, `right`, `up`, `down`), or an unrecognised action-bar slot `key` | `invalid_key` | silently uses the default key, with **no warning at all** | fix the key name |

### Action bars
| What you see | Code | What breaks if ignored | Fix |
|---|---|---|---|
| two slots in one `ActionBar` whose keys resolve to the same key | `duplicate_key` | they share one cooldown and one set of event names | give each slot a unique key |
| slots in two different `ActionBar`s that resolve to the same key | `cross_bar_duplicate_key` | cooldown and intent events are keyed by the slot key scene-wide, so a rule for one bar's slot also fires for the other | use distinct keys across bars |
| an unrecognised slot `gamepad_key` | `invalid_gamepad_key` | the slot never fires from the gamepad | fix the button name |
| one player with two or more slots on the same gamepad button | `same_player_gamepad_duplicate_key` | the slots collide | give each slot its own button |
| a slot with a `gamepad_key` whose owning player's prefab has no `inputs.gamepad_index` | `gamepad_key_without_gamepad_index` | the gamepad binding never fires (the keyboard key still works) | set `gamepad_index` on the player |
| a slot `cost` stat the owning player's prefab does not declare in `stat_templates` | `missing_player_stat_template` | the cost silently uses the shared global stat pool instead of that player's own | add the stat to the prefab's `stat_templates` |

### Flycam and spawning
| What you see | Code | What breaks if ignored | Fix |
|---|---|---|---|
| two or more `tags: ["flycam"]` entities in one scene | `duplicate_flycam_entity` | only the last one is used, the rest are discarded | keep one flycam entity |
| a prefab tagged both `"player"` and `"flycam"` | `flycam_player_tag_conflict` | it spawns as a camera only and its player parts never spawn | use `camera_mode: Flycam(...)` on a `"player"` prefab, or drop the `"player"` tag |
| a flycam prefab that sets a `model`, `shape`, `primitive` or `children` | `flycam_model_never_renders` | a flycam is camera-only, so that body never appears | remove the field, or spawn the body as a separate entity |
| a `join_prefab_keys` prefab with no `"player"` tag, or a primitive-shaped prefab | `unsupported_join_prefab` | hot-join refuses the prefab at runtime | use a GLB (Actor-kind) player prefab |
| a primitive-shaped player prefab in a scene that has `terrain` | `unsupported_primitive_player_on_terrain` | primitive players are not supported on terrain yet | use a GLB (Actor-kind) player prefab, or remove the terrain |
| `{new_id}` used anywhere except a `Spawn` action's `id` | `misplaced_new_id_token` | it is not substituted and appears as literal text at runtime | move it into the `Spawn` `id` |
| a `PlayAnimationOn` `start_at_fraction` outside `[0.0, 1.0]` | `animation_start_at_fraction_out_of_range` | it is a fraction of the clip, not seconds | use a value between 0 and 1 |
| a prefab `stat_label`/`world_stat_bar` keyed `{self}.<stat>` with no matching `stat_templates` entry | `missing_stat_widget_template` | the widget renders empty with no further warning | add the stat to the prefab's `stat_templates` |

### Players, items and dialogue
| What you see | Code | What breaks if ignored | Fix |
|---|---|---|---|
| two players in one scene (or reachable through `join_prefab_keys`) with the same `gamepad_index` | `duplicate_gamepad_index` | two players fight over one controller | give each a distinct index |
| a `join_prefab_keys` slot past the supported player count | `unreachable_join_slot` | that slot can never join | remove it |
| a joinable slot with no `player_{slot+1}_start` entry in the scene's `spawn_points` | `missing_reference` | the joining player spawns next to player 1 | add the spawn point |
| a catalog file (`assets.ron`, `prefabs.ron`, `stats.ron`, `items.ron`) that fails its own rules, e.g. an empty model path, a `Foliage` prefab missing its `foliage` block, a stat with `min > max`, an item with `max_stack: 0` | `invalid_asset_catalog` / `invalid_prefab_catalog` / `invalid_stat_catalog` / `invalid_item_catalog` | the catalog misbehaves at runtime | fix the entry named in the message |
| the `.project.ron` itself invalid (`schema_version`, or `max_frame_delta_secs` outside about 0.03 to 60 seconds) | `invalid_project_config` | a crash, or permanent slow motion | fix the field |

### Strict-only checks (`--strict`)
| What you see | Code | Meaning |
|---|---|---|
| a prefab, effect, audio or decal key defined but never used | `unused_prefab` / `unused_effect` / `unused_audio` / `unused_decal` | dead data; delete or use it |
| text containing a non-ASCII character in a `Label`/`Button`/entity `label:`/`world_labels:`/dialogue text | `non_ascii_char_in_text` | renders as a box (the built-in font is ASCII-only); use plain ASCII (hyphen, straight quotes) |
| a `ui.button_pressed:<trigger>` binding that no button, key or gamepad binding can ever fire | `orphan_binding` | dead code left over from a scene rewrite |
| a catalog file exists at its default location but the `.project.ron` field is unset | `unset_catalog_path_with_convention_file` | the runtime loads **nothing** for that catalog even though `validate` read it |
| `logic/state_machine.ron` exists but `state_machine_path` is unset | `unset_logic_path_with_convention_file` | the file is checked by no tool and loaded by the runtime never |
| a `Fixed(...)` camera with both or neither of `look_at`/`look_at_entity` | `camera_mode_fixed_ambiguous_look_at` / `camera_mode_fixed_missing_look_at` | working but ambiguous |
| a player's jump apex cannot clear its ground sensor | `jump_cannot_clear_ground_sensor` | the jump may never leave the ground; see `MovementConfig` in [20_data_formats.md](20_data_formats.md) |
| `max_walkable_slope_deg` outside `(0, 90]`, a negative `coyote_time_secs`, or a coyote time longer than the jump | `invalid_walkable_slope_limit` / `negative_coyote_time_secs` / `coyote_time_exceeds_jump_airtime` | the setting is ignored or masks the whole jump |
| `label_depth_scale.reference_distance` far outside the camera's radius range | `label_depth_scale_reference_distance_outside_camera_range` | depth scaling may never engage |
| two player prefabs in a scene with the same `player_index` (including both omitting it, which means `0`) | `duplicate_player_index` | identical P{n} label/colour and a shared target-ring layer |
| a `Group` whose `justify_content` spreads space but whose main axis is `Auto` | `inert_justify_content` | the setting does nothing; give it a `Px`/`Percent` size |
| a `Group` with `clip: true` and both axes `Auto` | `inert_clip` | the setting does nothing |
| an `Auto`-sized `Group` whose children are all `absolute: true` | `collapsed_group` | the group has zero size |
| a `Percent` size under an `Auto` parent (or a `Percent` group directly in a `ui_panel:` with no width/height) | `percent_under_auto` | resolves against the parent's content size |
| an `ActionBar`/`DialoguePanel`/`InventoryPanel`/`ShopPanel`/`ContainerPanel` inside a `Group` | `panel_nested_in_group` | its `position:` is measured from the group, not the screen; move it to the top level |

## 5. Inspect assets before you author

Use these to find the exact names and numbers you need to write RON, without opening the engine.

```bash
ironhold inspect glb     assets/shared/models/creatures/orc-enemy.glb
ironhold inspect texture assets/shared/textures/decals/circle_filled.png
ironhold inspect audio   assets/shared/audio/boulder/boulder-push1.wav
```

- **`inspect glb`**: animation clip names and durations (copy the names into an `AnimationPolicy`), mesh names with
  vertex/triangle counts, materials and root scene nodes.
- **`inspect texture`**: dimensions, format and channels, file size — catches oversized textures before they bloat
  the web download. PNG, JPEG, WebP, GIF, BMP and TIFF; AVIF is not supported.
- **`inspect audio`**: format, **duration**, sample rate and channels. Use the duration to set a sound-synchronised
  `delay_secs` in `EmitEventAfterDelay`. Reads WAV and MP3 only: it cannot read OGG, the format
  [20_data_formats.md](20_data_formats.md) recommends for music, so check OGG files in an audio editor.

## 6. Looking things up

```bash
ironhold stats  assets/projects/particles_demo/        # a one-screen project summary
ironhold query prefabs assets/projects/particles_demo/ --keys-only
ironhold query prefabs assets/projects/particles_demo/ --filter kind=actor
ironhold query effects assets/projects/particles_demo/ --filter additive=true
ironhold query scenes  assets/projects/3rd_person_game_demo/
ironhold query logic   assets/projects/3rd_person_game_demo/
ironhold query actions assets/projects/3rd_person_game_demo/
ironhold query events  assets/projects/3rd_person_game_demo/
```

- `stats`: scene/prefab/effect/rule counts, catalog sizes and project size on disk.
- `query prefabs|effects`: the catalog keys you can reference (`--keys-only` for one per line; `--filter` by kind,
  tag, behavior, additive, priority, layers, sprite).
- `query scenes|logic|actions|events`: every scene (entities, UI elements, player, overlay), the state machine's
  shape, every action type used and where, and every event trigger and what it fires.
- Any command accepts `--json` (placed **before** the command: `ironhold --json stats …`) for machine-readable
  output. `watch` ignores it.

## 7. "`validate` is clean but it still breaks"

`validate` cannot see everything. Known blind spots:
- an action-bar slot's own `do_actions` are not read at all, so effect, audio, prefab, item and spawn-point keys,
  `{token}` use and path casing inside a slot go unchecked, and a scene reachable **only** through a slot is not
  parsed or cross-checked;
- values that depend on runtime state (an enemy's behaviour, physics feel) are not checked at all;
- a button's **label text** is checked only for non-ASCII characters (and only with `--strict`), not for fit — text longer than its `size:` box
  overflows visibly (see the sizing note under `Button` in [20_data_formats.md](20_data_formats.md));
- anything the browser does differently from your machine (autoplay audio, GPU limits).

When something misbehaves with a clean report, open the browser console first — the engine logs a warning for most
of what the tool cannot see.
