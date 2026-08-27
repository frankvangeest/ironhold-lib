---
name: animation-hot-path
description: animation_playback_system / animation_resolver_system per-frame cost model, bevy_animation 0.18 paused-clip behavior, and the frozen-corpse waste it creates
metadata:
  type: project
---

Two per-frame systems, both Update, both iterating every entity with an `AnimationPolicyComponent`:
`animation_playback_system` (capabilities/animation.rs) and `animation_resolver_system`
(capabilities/animation_resolver.rs). Entity count = GLB players + GLB NPCs + (since
`dynamic_animation_control`) corpse props.

**bevy_animation 0.18: a PAUSED clip is still fully evaluated every frame.** `animate_targets`
(bevy_animation-0.18.0/src/lib.rs ~1029) only skips *event triggering* for
`active_animation.paused` (line ~1151); curve sampling and the bone `Transform` writes happen
regardless. So `freeze: true` (`PlayAnimationOn(start_at_fraction:, freeze:)`) does NOT stop
per-frame work — a frozen pose costs the same as a playing one, forever, plus it re-dirties every
bone `Transform` each frame so `propagate_parent_transforms` can no longer skip the skeleton
(which it *did* skip when the mesh had no `AnimationGraphHandle`). On wasm `par_iter_mut` is
effectively serial.

Concrete sizes measured 2026-08-26: zombie-01.glb = 42 skin joints; 6 monster slots in
3rd_person_game_demo/main.scene.ron ⇒ ≤6 coexisting corpses (ids reused per slot), each holding a
frozen pose for the full 300 s `SetDespawnTimer`. ~0.2–0.5 ms/frame worst case. The available
optimization (logged, not implemented): once the freeze has been applied, remove
`AnimationGraphHandle` from the player entity — `animate_targets`' `players.get(player_id)` then
bails at the cheapest possible early-out and the baked bone transforms hold the pose.

**Graph size is policy-driven, not GLB-driven — this is the thing that keeps it cheap.** Graph init
only calls `graph.add_clip` for names actually referenced by the policy (`base.*` + `clips` +
`overrides[].clip`), not for every merged clip. The corpse policies name one clip, so their graph is
exactly 1 node even though `animation_sources: ["anim_zombie","anim_locomotion","anim_hit_death"]`
merges 28 clips (locomotion.glb 17, hit_death.glb 8, zombie.glb 3). `animate_targets` iterates ALL
graph nodes per animation target, so a policy that references 30 clips would cost 30× more per bone
per frame. Keep single-purpose policies single-clip.

**Pre-existing per-frame allocation:** resolver step 4 clones a `String` for `chosen_clip` every
frame for every animated entity (needed because `active.clip` is borrowed while step 4b wants
`active.clear()`). Adding animation policies to props amplifies it linearly. Fix if it ever matters:
resolve to a `&str`, decide the 4b fallback into a bool, clone only at the `current != chosen` write.
Step 5 also writes `transition_ms`/`should_loop` unconditionally (marks `AnimationController`
`Changed` every frame) — harmless today since nothing filters on it, same latent-footgun class as
[[npc-locomotion-bridge]]'s `is_grounded`.

**No render-pipeline consequence from adding an animation policy to an existing GLB prop.**
bevy_gltf already inserts `AnimationPlayer` + `AnimationTarget`/`AnimatedBy` for any GLB that has
animations (loader/mod.rs ~1024), and `SkinnedMesh`/joint-matrix extraction already ran every frame.
Animation only writes `Transform`s — no new shader def, no new pipeline key, so no WebGPU first-draw
compile stall. Contrast with [[world-icon-stat-bar]], where a genuinely new pipeline was introduced.

`animation_playback_system` requires `&ActiveOverride` as a query term (added by this feature). Both
production `AnimationController` spawn sites (entity_spawner.rs prefab-instance + player paths)
insert `ActiveOverride::default()`, so it's safe — but a future third site that forgets it loses ALL
animation silently. `#[require(ActiveOverride)]` on `AnimationController` is the Bevy 0.18 fix.
