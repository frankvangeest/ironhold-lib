---
name: RON parse-failure diagnostics by file type
description: What a designer actually sees when a RON file fails to parse — ron 0.11 produces an excellent "Unexpected field named X in Variant" message, but the engine's own per-file handling ranges from good (catalogs) to none at all (.behavior.ron), and blast radius is always the WHOLE file
type: project
---

**The RON error text itself is good.** `ron 0.11`'s `NoSuchStructField` Display is:
`Unexpected field named `x` in `Y`, expected one of `a`, `b`, or `c` instead`. For an **enum struct
variant** (every struct-form `Action`), `de/mod.rs::struct_variant` captures `last_identifier` and
rewrites `outer`, so `Y` is the **variant name** (`PlayAnimationOn`), not a generic struct name.
`ImplicitRonLoader` (`schema/ron_loader.rs`) wraps it as `RON parse error: {line}:{col}: …`, and
Bevy's asset server logs `Failed to load asset '{path}' with asset loader '{loader}': {error}` at
`error!`. Net result in the browser console — genuinely actionable, names file, line:col, action,
bad field, and the valid field list. Do NOT flag "the error will be opaque"; verify first.

**The engine's own per-file handling is inconsistent** (`runtime/scene_manager/project_loader.rs`,
phase 2, ~lines 153-225):
- `asset_catalog` / `prefab_catalog` / `stats` / `items` → `error!` **with path and `{e}`**. Good.
- `model_fixes` / `state_machine` → UPDATED (verified 2026-09-23; re-checked 2026-09-29 —
  `rules` was a third member of this tier and no longer exists as a file type at all following
  `rules_to_state_machine_consolidation`, see [[fsm-designer-traps]]): both now `error!` with
  path + `{e}`; `state_machine`'s message additionally suffixes "no project logic (global_on
  rules, states, transitions) will run" (`project_loader.rs`). The old warn!-without-path tier is
  gone for both.
- `.behavior.ron` → FIXED (verified 2026-10-01): `resolve_pending_behaviors_system`
  (`entity_spawner.rs` ~572) now `error!`s with spawn id + path + `{e}` ("this entity will never
  run its behavior") and drops `PendingBehavior`. `.dialogue.ron` likewise (`dialogue.rs` ~139).
- Still silent on-screen: a `.project.ron` parse failure (e.g. leftover `rules_path:`) hangs the
  loading screen forever, console-only error (backlog Bugs, found 2026-09-28).

**Blast radius is always the entire file**, never the one bad line. One typo in
`logic/state_machine.ron` = every binding in the project stops firing. Any doc note about strict
parsing MUST say this — the designer's symptom is "all my logic died", not "one action
misbehaved".

**Why:** designers working from `assets/` + a WASM build have the browser console as their only
diagnostic channel (docs/20 already states this explicitly around line 2017 — that's the house
style to match). A strictness change that makes failures *louder in Rust* can still land *silent
for the designer* depending on which loader path the file goes through.

**How to apply:** when reviewing any change that makes RON parsing stricter, (1) confirm the ron
error text rather than assuming, (2) trace which of the three handling tiers above the affected
file type falls into, (3) require the doc note to state whole-file blast radius + quote the
verbatim console line so it is greppable. Related: [[docs-lag-the-action-schema]],
[[ron-enum-double-paren-trap]].

**Action authoring locations (for "where does this apply" lists):**
`logic/state_machine.ron`, `behaviors/*.behavior.ron`, `dialogues/*.dialogue.ron` choice
`do_actions`, **`scenes/*.scene.ron`** (UI button `do_actions`, action-bar slots — shipped in
3rd_person_game_demo, stats_demo, primitive_world, local_coop_demo rooms 3/9/10) and
**`prefabs/prefabs.ron`** (`on_death_actions` in 3rd_person_game_demo, `SetCameraMode` in
entity_logic_demo). Doc notes routinely list only the first four.
