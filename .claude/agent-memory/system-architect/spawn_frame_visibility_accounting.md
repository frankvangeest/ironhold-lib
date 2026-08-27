---
name: spawn-frame-visibility-accounting
description: Bevy 0.18 frame accounting for newly spawned GLTF models — why any "hide after spawn" guard is structurally one rendered frame too late, plus the ThreadedAnimationGraphs one-frame gate on animate_targets
metadata:
  type: project
---

Verified against vendored `bevy_animation-0.18.0`, `bevy_scene-0.18.0`, `bevy_asset-0.18.0`
(2026-08-27) while investigating the corpse bind-pose flash.

## Schedule facts (0.18)

- Main schedule order: `First → PreUpdate → StateTransition → RunFixedMainLoop → Update →
  **SpawnScene** → PostUpdate → Last`.
- `bevy_scene`: `(scene_spawner, scene_spawner_system).chain()` in **SpawnScene**. So a `SceneRoot`
  inserted by an Update-schedule `Commands` call spawns its whole GLTF hierarchy **later the same
  frame**, and those meshes render that frame.
- `bevy_animation::AnimationPlugin`: `thread_animation_graphs → advance_transitions →
  advance_animations → animate_targets → …` all in **PostUpdate**, `.in_set(AnimationSystems)`,
  `.before(TransformSystems::Propagate)`.
  **Consequence:** a `transitions.play()` / `set_seek_time()` / `pause()` issued from an *Update*
  system IS evaluated into joint `Transform`s, propagated to `GlobalTransform`, and extracted to
  the render app **in that same frame**. There is no inherent Update→PostUpdate 1-frame pose lag.

## The "hide after spawn" anti-pattern (root cause class)

A system that hides an entity by querying for a component which was inserted by the **same
deferred command batch** that inserted `SceneRoot` can never beat the mesh's first render:

```
frame A Update:      spawn parent(Visibility::default(), Marker) + child(SceneRoot)  [deferred]
frame A end-of-Update: commands flush — entity + SceneRoot + Marker now live
frame A SpawnScene:  GLTF hierarchy spawned, Visibility inherited => VISIBLE
frame A PostUpdate:  visibility/transform propagate; no AnimationGraphHandle yet => BIND POSE
frame A render:      ***one guaranteed frame of visible bind pose***
frame A+1 Update:    the guard system finally sees Marker and inserts Visibility::Hidden
```
Add more frames if the guard also waits on an async asset (an `AnimationPolicy` RON, etc.).

**Rule:** anything that must never be seen in its pre-initialized state has to be spawned
`Visibility::Hidden` **in the same command batch as `SceneRoot`** (`runtime/model_spawner.rs`
currently spawns both parent and child `Visibility::default()`), and revealed later. Reveal-side
delay is cheap and safe; hide-side delay is unfixable after the fact.

**Corollary:** always pair a spawn-time hide with a bounded **failsafe reveal** (deadline +
`warn!`). Without one, "hidden until initialized" turns a 1-frame cosmetic flash into a
permanently-invisible entity whenever the GLB/policy never loads, a clip is missing, or the
`AnimationPlayer entity lost` recovery branch in `capabilities/animation.rs` re-hides and never
re-finds a player.

## ThreadedAnimationGraphs: animate_targets writes NOTHING the frame a graph is created

`animate_targets` (lib.rs:1061-1065) silently `return`s when `ThreadedAnimationGraphs` has no
entry for the graph's `AssetId`. That map is filled by `thread_animation_graphs`, which reads
`MessageReader<AssetEvent<AnimationGraph>>` and is scheduled **`.before(AssetEventSystems)`** —
while `Assets::<AnimationGraph>::asset_events` (the emitter of `Added`) is in `PostUpdate` in
`AssetEventSystems`, i.e. **after its only reader**. So a graph created with `graphs.add()` in
frame N's Update is only threaded in frame **N+1**'s PostUpdate; frame N renders the bind pose.

**Why it matters / latent trap:** `animation_playback_system`'s "reveal in the same call as the
first `play()`" is currently correct only by coincidence — our `AnimationTransitions` insert is
*also* deferred, so the first successful `play()` naturally lands on N+1, the same frame
`animate_targets` starts working. Eagerly inserting `AnimationTransitions::new()` at spawn (a
tempting way to delete the `animation.rs` "Waiting for AnimationTransitions" retry) would move the
first play to frame N and make the reveal precede the pose by one frame — reintroducing a bind-pose
flash through the reveal path. Keep the two deferrals, or gate the reveal on an observable
readiness signal instead of "the play call happened".

## Testing implication

The headless `cargo test` harness can prove `awaiting_reveal`/`Visibility` component *transitions*
but never the frame at which pixels are correct (no renderer, and per debug-detective memory no
`AnimationPlugin`). It CAN, however, assert frame accounting: spawn a prefab, run one `app.update()`,
and assert the parent's `Visibility` at end of frame A. That is the correct regression test for this
bug class. `test_web.py` screenshots cannot catch a single-frame flash.

**Cheapest way to prove/disprove any "is it a frame-ordering bug?" hypothesis here:** log
`Res<FrameCount>` at the spawn site, the hide, the play/seek, and the reveal. If `hide.frame >
spawn.frame`, the too-late-hide mechanism is proven without a playtest. See also
[[animation_seek_freeze_constraints]], [[wasm_pitfalls]].
