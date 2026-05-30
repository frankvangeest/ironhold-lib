---
name: particle-quality-budget-pattern
description: Six-touchpoint checklist for new global-state actions (like SetParticleQuality) that mutate a persistent runtime resource; backward-compat checklist for adding optional fields to EffectDef/LayerDef
metadata:
  type: project
---

The 2026-05-27 particle quality/budget review surfaced a clean reference pattern for two recurring shapes of change.

## Pattern A: New global-state Action that mutates a runtime Resource

When a new `Action` variant exists primarily to flip a designer-facing tuning knob (no entity ref, no scene side-effects), six touchpoints must all line up. `Action::SetParticleQuality(QualityLevel)` is the canonical example.

1. **`schema/actions.rs`** — single tuple variant `Foo(SomeEnum)`. Tuple form is required for the RON syntax `Foo(High)`; struct form would force `Foo(level: High)` and contradict project convention for single-payload variants. Doc-comment MUST state whether the action persists across `LoadScene`.

2. **`capabilities/{system}.rs`** — new `#[derive(Resource)]` with a `Default` impl that defines the engine-default value. Two consumers normally read it: the spawn/drain system, and the executor.

3. **`lib.rs`** — `init_resource::<NewResource>()` near the other particle/render resources. Without this, the first `Action` fires before the resource exists and panics on `ResMut` lookup.

4. **`runtime/scene_manager/action_executor.rs`** — match arm. For pure-tuning actions, this is typically a one-liner `spawn_params.field.level = level;` plus an `info!` log. No entity registry lookup needed.

5. **Decide persistence** — explicitly answer "does this reset on `LoadScene`?". Two correct patterns:
   - **Persists** (player setting): never reset. `ParticleQuality` is the example.
   - **Per-scene** (level design): reset in `scene_loader.rs` from a `Option<T>` field on `GameSceneV2`. `ParticleBudget` (set from `scene.particle_budget.unwrap_or(2000)`) is the example. The default value MUST match the `Default` impl on the resource — duplicated in two places, both must agree.

6. **`rewrite_self`** — pure global-state actions skip this. ONLY add a match arm here if the action carries an entity reference. `SetParticleQuality` is correctly absent.

Designer-reachability test: a designer can put the action in `rules.ron` on `scene.ready:{name}` and have it apply before the player can interact. They can also put it on a UI button (`ui.button_pressed:graphics_low`) for an in-game settings menu. Both paths must work with zero Rust changes.

## Pattern B: Adding optional fields to EffectDef / LayerDef without breaking projects

The particle catalog has `#[serde(deny_unknown_fields)]` on both `EffectDef` and `LayerDef`. ANY new field added to either MUST satisfy all of:

1. `#[serde(default)]` on the field — required so existing `assets.ron` files keep parsing.
2. If the field exists on BOTH structs (e.g. `quality`, `priority` are EffectDef-only here, but most particle fields are duplicated), it must be propagated in `From<&EffectDef> for LayerDef` — see `[[recurring_anti_patterns]]` particle entry. Forgetting this silently kills the entire effects catalog because `deny_unknown_fields` rejects the parse.
3. If the runtime consumes the field via a fallback (e.g. `scaled_count` falls back to global multiplier when `quality_override` is `None`), the doc-comment should state the fallback in the SAME terms the runtime uses, so designers can predict behaviour without reading Rust.

The 2026-05-27 review confirmed all three were satisfied for `quality`/`priority`. The `From<&EffectDef> for LayerDef` propagation is the most fragile of the three because the compiler does not flag the omission.

## Cross-cutting: optional-Option<String> entity fields need rewrite_self too

`Action::ProjectDecal` and `Action::SpawnEffect` both carry `entity: Option<String>`. `SpawnEffect` is in `rewrite_self`; `ProjectDecal` is not. Until ProjectDecal is added, designers can use it only with literal spawn IDs in `rules.ron` — not in reusable `.behavior.ron` files via `{self}`. This is the failure mode called out in `[[entity_targeted_action_pattern]]`. Flag this any time a review touches `ProjectDecal`.

## Scene-RON optional field pattern (GameSceneV2.particle_budget)

`particle_budget: Option<u32>` with `#[serde(default)]` is the model for new scene-scoped numeric tunables. Consumed in `scene_loader.rs` via `unwrap_or(DEFAULT)`. When suggesting similar fields (e.g. ambient cap, decal cap, fog density), prefer this shape over a non-optional with a custom `default_*` fn — `Option<T>` is easier for designers to reason about ("omit = engine default") than `#[serde(default = "default_X")]` (where they have to guess the value).
