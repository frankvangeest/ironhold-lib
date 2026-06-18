---
name: AnimationPolicy doc gaps (sources + PlayAnimationOn)
description: animation_sources field is undocumented in the AnimationPolicy section, and PlayAnimationOn is absent from the Available actions table; the clip-vs-id distinction is unexplained
type: project
---

The AnimationPolicy section of `docs/20_data_formats.md` (## prefabs/animation/*.ron, ~line 1676) has two persistent designer-facing gaps:

1. **`animation_sources` is undocumented as a field.** It appears only in a prose warning (~line 1682: "Without it, `animation_sources` retargeting silently does nothing") and is ABSENT from both the example RON block (~1685-1719) and any field table. Yet it is load-bearing: in `3rd_person_game_demo/prefabs/animation/player_policy_human.ron` a designer MUST add the GLB source (e.g. `anim_magic`, `anim_hit_death`) to `animation_sources` before a clip in that source can be referenced. The list entries are catalog keys pointing at animation GLBs. A designer cannot discover this from docs — they will add a clip to `clips:`/`overrides:` and silently get no animation because the source GLB was never loaded.

2. **`PlayAnimationOn` is absent from the "Available actions" table** (~line 1776-1809). The table lists only `PlayAnimation("id")` (semantic ID, no target — uses the implicit player). But scenes/state machines target a specific entity with `PlayAnimationOn(target: "player_01", clip: "attack_light")`. This is the form actually used in `main.scene.ron` action bar slots and `state_machine.ron` death rule. (Already tracked in [[Docs lag the action schema]] as a missing action; this memory adds the clip-vs-id nuance.)

3. **`PlayAnimation(id)` vs `PlayAnimationOn(clip:)` are semantically different and the docs never contrast them.** `PlayAnimation` takes a *semantic override id* (an `overrides[].id`). `PlayAnimationOn` takes a `clip:` arg — designers reasonably assume this is also a semantic id, and in the demo it happens to work because each override's `id` equals a `clips:` alias of the same name. Whether `clip:` resolves against `overrides[].id`, the `clips:` map alias, or the raw glTF clip name is NOT documented. Needs verification against the executor before asserting which one it is. Flag as "needs verification" in reviews.

**How to apply:** When reviewing animation changes, check that (a) any new GLB referenced by clips/overrides is in `animation_sources`, (b) `PlayAnimationOn` has a row in the actions table, and (c) the clip-vs-id resolution is documented. Canonical full example: `3rd_person_game_demo/prefabs/animation/player_policy_human.ron`.
