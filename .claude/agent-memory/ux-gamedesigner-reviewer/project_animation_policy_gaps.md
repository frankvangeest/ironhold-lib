---
name: animation-policy-doc-gaps
description: AnimationPolicy has no top-level field table in docs/20; clips-alias looping semantics undocumented; historical record of which gaps closed when (PlayAnimationOn row, clip resolution order, animation_sources)
metadata:
  type: project
---

Section: `docs/20_data_formats.md` ▸ `## prefabs/animation/*.ron — AnimationPolicy` (~line 3510).

**CLOSED as of 2026-08-26 (`feature/dynamic-animation-control`)** — do not re-flag these:
1. `animation_sources` is now explained in a comment inside the example RON block (~3532-3535:
   "A clip referenced in `clips:` or `overrides[].clip` must come from a GLB listed here, or it
   silently won't play"). Still no field-table row, but the footgun is now discoverable.
2. `PlayAnimationOn` now HAS a row in the "Available actions" table (~line 3711), with target,
   clip, `start_at_fraction`, `freeze` all described.
3. Clip-vs-id resolution IS now documented (~line 3590): **override `id` → `clips:` alias → raw
   glTF clip name**, in that order. Cite this instead of flagging it as unknown.

**STILL OPEN:**
- **There is no top-level `AnimationPolicy` field table anywhere in docs/.** `default_transition_ms`,
  `animation_sources`, `base` (and its four required sub-fields `idle`/`walk`/`run`/`jump_loop`),
  `clips`, `overrides` are documented ONLY by the example RON block + prose. Only
  `AnimationOverrideDef` gets a real table (~3594). A designer cannot see which fields are
  optional.
- **Whether a `clips:`-alias playback loops is undocumented.** `clips:` entries are a bare
  `name → glTF clip` map with no `looping` field, unlike `overrides[]` which has an explicit
  `looping: bool`. `dynamic_animation_control`'s third example asserts a `clips:` alias
  ("walk") keeps looping after a mid-clip seek — that behaviour is asserted in RON and in a
  scene label but stated nowhere in docs/. Flag as "needs verification" on any animation work.
- **How to UN-freeze a frozen clip is undocumented.** `PlayAnimationOn(..., freeze: true)` has no
  documented inverse. Whether a later `PlayAnimationOn` with `freeze: false` (or a different
  clip) resumes/replaces it is not stated and has no shipped example.

**Reserved override IDs** `jump_enter`/`jump_exit` are fired automatically for every player
prefab; a locomotion-only policy still needs both or you get per-jump WARN spam. Smallest working
example: `local_coop_demo/prefabs/animation/player_locomotion.ron` (docs/20 ~3519-3526).

Canonical full policy: `3rd_person_game_demo/prefabs/animation/player_policy_human.ron`.
Canonical seek/freeze policy: `dynamic_animation_control/prefabs/animation/zombie_policy.ron`
(deliberately carries an override id, a `clips:` alias, AND raw-name-reachable clips so all three
resolution branches are exercised in one file).
Related: [[docs-lag-actions]], [[dynamic-animation-control-demo]], [[corpse-loot-v2-pattern]].
