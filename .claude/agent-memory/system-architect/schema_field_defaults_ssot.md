---
name: schema-field-defaults-ssot
description: House convention for a schema struct needing both per-field serde defaults and a hand-built default value (per-field default fns + hand-written impl Default calling them, per FlyCamDef/MovementConfig); why derive(Default) is a footgun on key-binding structs; and why defaulting a field is the inverse of a tightening change
metadata:
  type: project
---

**The convention.** When a schema type needs *both* per-field RON defaults and a hand-built default
value for a runtime module, the house pattern is: per-field `#[serde(default = "default_x")]` +
a **hand-written** `impl Default` whose every field calls those same `default_x()` fns. The runtime
module then calls `T::default()` instead of writing its own struct literal. Established instances:
`FlyCamDef` (`schema/catalog.rs` struct + `impl Default`, consumed by `scene_loader.rs` as
`FlyCamDef::default()`) and `MovementConfig` (`schema/catalog.rs`). A weaker precedent exists for
pulling a single fn cross-module: `entity_spawner.rs`'s `default_camera_config()` does
`fov: crate::schema::player::default_fov()` (that one fn is `pub(crate)` for exactly this) while
still duplicating `initial_pitch`/`look_speed` literals beside it.

**Why it matters beyond DRY:** an exhaustive hand-written `impl Default` makes "added a field and
forgot its default" a **compile error in one place**. Without it, that mistake ships as a
designer-facing hard RON parse error (`missing field "forward"`) discovered at authoring time — the
exact bug `feature/input_map_defaults` (2026-09-10) existed to fix on `InputMap`'s 7 movement/jump
fields.

**Counter-instance to watch:** `InputMap`'s hand-built default is a full 22-field struct literal
(`entity_spawner.rs::default_input_map()`), and the same value set is hand-written ~17 more times
across `crates/ironhold_core/tests/` (per-file `test_input_map()`/`input_map()` helpers plus inline
literals). Those copies already drift benignly (`strafe_mouse_button: None` in some, `Some("Left")`
in others). An `impl Default for InputMap` collapses all of them.

**Never `#[derive(Default)]` on a key-binding struct.** `""` matches no arm in
`InputMap::parse_key`, so a derived default yields bindings that are silently *unbound* with no
warning anywhere — strictly worse than the parse error you were removing.

**Defaulting a field is the inverse of a tightening change, and has its own cost.** A required
field's `missing field "X"` error doubles as an accidental **typo detector**: a misspelled field
name is silently dropped by serde, then caught by the resulting missing-field error. Add
`#[serde(default)]` and that typo becomes fully silent unless the struct carries
`#[serde(deny_unknown_fields)]`. So a defaulting change should be paired with a
`deny_unknown_fields` decision on the same struct. As of 2026-09-10 `InputMap` has no
`deny_unknown_fields`, while its parent `PrefabComponents` (`schema/catalog.rs`) and its
file-siblings `AnimationPolicy`/`BaseAnimations`/`AnimationOverrideDef` (`schema/player.rs`) all do
— so a typo at the `components:` level is caught but one *inside* `inputs: (...)` is not.
See [[schema-tightening-blast-radius]] for the opposite direction.

**Reviewer heuristic that paid off here:** `docs/20_data_formats.md`'s per-field **Default** column
can promise a default the parser doesn't implement. Its `InputMap` table already listed
`"KeyW"`/`"KeyS"`/…/`"Space"` as defaults for the 7 fields that were in fact required, so that fix
was a code-vs-documented-contract reconciliation, not an ergonomics nicety. Check the docs↔code
default agreement in **both** directions. Related: [[cli-runtime-mirror-check-pairs]].
