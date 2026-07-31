---
name: gamepad-hot-join-pattern
description: global/scene_gamepad_bindings mirror the key-bindings 3-merge-site pattern; PendingJoinGamepad is a frame-scoped detection→executor courier; the "any-trigger capture" hole and the unclaimed-only gate footgun
metadata:
  type: project
---

Gamepad-triggered hot join (`planning/features/gamepad_hot_join.md`, reviewed 2026-07-29 — ALIGNED).
Follow-up to [[hot_join_pattern]]; see also [[keybinding_parse_key_vocabulary]],
[[local_coop_pattern]].

**Bindings-map pattern (canonical, copy this for any future global-input map):** a new
project-level + scene-level `HashMap<String,String>` binding map needs a `Project*` + `Loaded*`
resource pair and **three** merge sites, not one:
- `project_loader.rs` inline-config branch (~line 129) — `insert_resource(Project…)` + `Loaded…`
- `project_loader.rs` external-files branch (~line 276) — same two inserts
- `scene_loader.rs` Replace-mode branch (~line 143) — per-key overlay of `scene_*` on top of
  `project_*`, then `*loaded_* = Loaded…(effective)`
Miss any one and bindings bleed across scenes on only one loading path. Overlay mode deliberately
does NOT rebuild (mirrors `LoadedKeyBindings`).
`SceneV2Params` is where the two new gamepad resources had to go — `spawn_scene_v2` is at Bevy's
16-bare-param ceiling (same reason `dynamic_stat_ui_queue` lives there).

**`PendingJoinGamepad(Option<Entity>)` — frame-scoped courier pattern.** Distinct from
`ActiveSplitSlotCount`/`DynamicSplitConfig` (persistent, set on scene load, cleared on LoadScene)
and from `PendingEntitySpawns` (a queue). It is reset to `None` unconditionally at the top of
`unclaimed_gamepad_trigger_system` (before the `AppState`/empty-map early returns) and `take()`n by
the `Action::JoinPlayer` executor arm, so it needs no LoadScene clear. Ordering that makes it work:
detection is `.before(message_interpreter_system)`, executor is chained after the interpreter.
This is the right shape for "detection system must hand an Entity identity to an executor that the
Action variant itself can't carry" — reuse it rather than adding a payload to the Action.

**No hardcoded `"join"` magic string — and this is structurally forced.** The engine cannot know
which trigger means join (trigger→Action mapping lives in `rules.ron`), so detection captures the
pad for *any* `LoadedGamepadBindings` match. Positive alignment, but it creates a real hole:
**within a single frame, the captured pad is the lowest-sorted-index pad that produced ANY bound
press, not necessarily the pad that produced the join trigger.** So pad A pressing a
`"Start":"toggle_pause"` binding + pad B pressing `"South":"join"` in the same frame binds the
joiner to pad A. The plan's staleness fix only closes the *cross-frame* case (and there is a test
for that: `test_pending_join_gamepad_is_frame_scoped_not_sticky`). Rare, benign-ish (input is
additive so the joiner isn't locked out), logged as a suggestion — but re-flag it if anyone extends
`*_gamepad_bindings` beyond the single-join use case.

**FOOTGUN — `global_gamepad_bindings` is NOT the general gamepad analogue of
`global_key_bindings`.** Every match is gated on the pad being *unclaimed* (no live player's
`InputMap.gamepad_index`, and no undrained `is_hot_join` `PendingEntitySpawns` entry, points at that
sorted index). There is no RON opt-out of that gate. Consequence: a designer binding
`"Start":"toggle_pause"` project-wide gets a pause that silently stops working the moment that pad
is claimed — and whose behavior flips based on whether an unrelated player prefab happens to author
`gamepad_index`. Documented in the schema doc comments and `docs/20_data_formats.md`, but the field
*name* oversells it. There is still **no** RON path for a joined player's gamepad button → arbitrary
trigger (`InputMap` only exposes jump/run/interact/target_next).

**Input is ADDITIVE, not exclusive** — verified against `input_translator_system` (keyboard read
first, gamepad `+=`/`||` on top) and `camera_orbit_system`. So a gamepad-joined player keeps their
join prefab's authored keyboard scheme. `local_coop_demo/prefabs/prefabs.ron` lines ~335-336 still
claim the opposite ("the keyboard bindings above are simply ignored while gamepad_index is set") —
that pre-existing comment is factually WRONG and now contradicts room8's own comment and
`docs/20_data_formats.md`. Flag it on any gamepad-input touch until fixed.

**Validation:** dual runtime-`warn!` (project_loader) / `load_errors` (scene_loader) plus strict
`ironhold_cli validate` `error_type: "invalid_binding"` for both maps — matches the established
runtime-lenient/CLI-strict split. The CLI reuses `ironhold_core::schema` directly (no duplicated
schema copy), so schema field adds are free there; only `validate.rs` needed the new checks (no new
`Action` variant, so `query.rs`'s exhaustive match was untouched).
