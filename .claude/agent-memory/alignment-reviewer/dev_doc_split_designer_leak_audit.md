---
name: dev-doc-split-designer-leak-audit
description: How to review a docs/CLAUDE.md reorganisation for designer-facing facts that end up only in dev-only places; known stale designer-doc contradictions found during the 2026-10-08 core CLAUDE.md split review
metadata:
  type: project
---

Reviewing a CLAUDE.md/docs split (feature/core_claude_md_split, 2026-10-08): the split left
`docs/dev/moved-sections-index.md`, which maps every old `src/CLAUDE.md` block (`b:N`) to its new home. Use it
as the checklist. For each block whose destination is only a folder CLAUDE.md / docs/dev / "cut", grep
docs/20, docs/30, docs/15 for the designer-facing part.

**Why:** the old core CLAUDE.md often held the only *correct* statement of a designer-visible behaviour, and
designer docs had drifted. Moving it to dev-only places leaves designers with only the stale text.

**How to apply:** check the field-reference row for a caveat, not just whether the topic is mentioned somewhere.
Several caveats exist only inside the docs/30 lootable-corpse walkthrough, which a designer reading the
PrefabDef/Actions tables never sees.

Known gaps found then (re-verify before citing):
- `{self}` in dialogue choice `do_actions`: code substitutes it (`dialogue.rs::substitute_self_in_action`,
  incl. Spawn id/spawn_point/at_entity). docs/20 dialogue table says yes; docs/30 `{self}` section and the
  docs/20 `Spawn` row say no. Three-way contradiction.
- Particles: the pool renderer picks Add vs Blend from `additive:` only (`particle_renderer.rs` `group_key`),
  flame from `uv_distort`/`uv_scroll_speed`. docs/20 still says "no sprite → AlphaMode::Add" / "additive has no
  effect without a sprite" / `FlameParticleMaterial` (the pool uses `PoolFlameMaterial`). The warmup recipe
  inherited this.
- Caveats only in the corpse walkthrough, not the field rows: `trigger_zone` mass on Dynamic bodies,
  `interactable` fires for every in-range entity, `EmitEventAfterDelay` ticks while paused (catch in `global_on`).
- RON struct-vs-tuple action syntax rule now lives only in `schema/CLAUDE.md`.

Related: [[new-id-token-pattern]], [[dialogue-system-pattern]], [[lootable-corpse-pattern]], [[particle-quality-budget-pattern]].
