---
name: {self} substitution pattern for entity-targeted actions
description: New entity-ID-targeted actions follow a {self} substitution pattern that is now well-documented in 30_runtime docs
type: project
---

Actions that target an entity by spawn ID (`ShowDamagePopup`, `SetEntityVisible`, `EmitEventAfterDelay`, `ModifyStat`, `SetStat`, `PlayAnimationOn`, `Despawn`, `Spawn { id }`, `EmitEvent`) accept `{self}` inside their target string when used in `.behavior.ron` files. The interpreter substitutes it with the running entity's spawn ID at execution time.

**Why:** This is how behavior files are reused across multiple instances (e.g. `dummy_01`, `dummy_02`, `dummy_03` share the same `attack_dummy.behavior.ron`). Without `{self}`, the behavior would only work on a single hardcoded entity.

**How to apply:**
- Docs section `docs/30_runtime_events_and_logic.md` "Entity FSM (per-entity behavior)" has the canonical `{self}` reference table — when reviewing new entity-targeted actions, ensure both the action's row in the action table AND the `{self}` substitution table list it.
- A complete list of supported `{self}` targets is in `crates/ironhold_core/src/CLAUDE.md` — designers won't see this, but it's the cross-check for docs completeness.
- Canonical worked example is `assets/projects/primitive_world/behaviors/attack_dummy.behavior.ron` (respawn loop demonstrating hide+delay+restore over `Despawn`+`Spawn`).
