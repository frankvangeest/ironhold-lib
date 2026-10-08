# Animation resolver and playback pipeline

Field ownership between `animation_resolver_system` and `animation_playback_system`, seeks and frozen poses.

> Moved from `crates/ironhold_core/src/CLAUDE.md` on 2026-10-08 (the core CLAUDE.md split). The hard rules for this area stay in the folder `CLAUDE.md` files; this page keeps the long-form reference and history verbatim.

## Animation resolver/playback pipeline, field-ownership table

<!-- b:622 -->

### Animation resolver/playback pipeline (`capabilities/animation_resolver.rs` + `capabilities/animation.rs`)

Two-stage pipeline inside `lib.rs`'s single `Update` `.chain()` — **not adjacent**: `animation_resolver_system` is first and `animation_playback_system` last, with the 12 per-mode camera systems (through `camera_blend_system`) between them:
`animation_resolver_system` (turns `LocomotionState` + queued `AnimationRequest`s into a single
`AnimationController.current`) → `animation_playback_system` (drives the real Bevy
`AnimationPlayer`/`AnimationTransitions` from that). **Field ownership is split between the two,
not fully owned by either** — despite the resolver's own doc comment saying it's the sole writer
of `current`, `animation.rs`'s missing-node-index recovery path also writes it (a last-resort
fallback to `base.idle`, not a normal write). The full split:

| Field | Owner | Notes |
|---|---|---|
| `current`, `transition_ms`, `should_loop` | resolver (write); playback (idle-fallback exception) | |
| `pending_seek` | resolver sets; playback clears | see the `pending_seek` section of this page |
| `last_played`, `graph_initialized`, `node_indices`, `last_player_entity` | playback | |

## `pending_seek` purpose

<!-- b:638 -->

**`AnimationController.pending_seek`** (`planning/features/done/dynamic_animation_control.md`) exists
because playback only re-triggers `transitions.play()` on `current != last_played` — a no-op for
"re-seek the *same* clip to a different fraction" (`PlayAnimationOn(..., start_at_fraction: ...)`
called twice in a row against an already-current clip). The resolver sets `pending_seek = true`
whenever it accepts a queued request that carries `start_at_fraction`/`freeze` — deliberately
**not** for an ordinary re-request with neither (e.g. rapid re-presses of a plain
`attack_light` override), which keeps its pre-existing behavior of not restarting the clip. Only
seek/freeze requests need the forced replay.

## Spawn-already-posed needs a minimal `AnimationPolicy`

<!-- b:677 -->

**Spawning an entity already posed mid-clip** (not just holding a death pose after a full
playthrough — see `docs/20_data_formats.md`'s "Spawn-already-posed pattern") needs its own
minimal `AnimationPolicy` file with `base.idle`/`walk`/`run`/`jump_loop` all pointing at the
target clip, not a reuse of a live character's full policy — see
`prefabs/animation/corpse_policy_zombie.ron`. Reusing the full policy leaves three independent
fallback paths (no active override, missing node index, graph validation failure) that all land
on `base.idle`, which for a "should look dead" entity is a strictly worse degraded state than for
a live one.
