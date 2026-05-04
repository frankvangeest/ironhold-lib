# Investigation: T-Pose / Animation Break Bug
(date: 2026-04-17)

**Status:** Partial fix applied; root cause of T-pose not yet confirmed from runtime logs.  
**Affected projects:** `3rd_person_game_demo` (player), `quick_scene` (giant NPC)  
**Symptoms:**
- Player falls from spawn height, plays jump animation, lands → T-pose. Animations stop working for the rest of the session.
- Giant NPC in quick_scene always in T-pose (never animates). Quick-scene player is unaffected.

---

## Files read during investigation

| File | Why |
|---|---|
| `capabilities/animation.rs` | Core playback system — edited to add logging |
| `capabilities/animation_resolver.rs` | Picks which clip to play each frame |
| `runtime/scene_manager/entity_spawner.rs` | Spawns prefab instances + `animation_policy_loader_system` |
| `runtime/scene_manager/scene_loader.rs` | Scene transition cleanup; **bug found here** |
| `runtime/scene_manager/action_executor.rs` | `Action::PlayAnimation` broadcast logic |
| `capabilities/player.rs` | Emits "jump_enter"/"jump_exit" override pushes |
| `assets/projects/quick_scene/prefabs/animation/giant_npc_policy.ron` | Giant NPC animation policy |
| `assets/projects/3rd_person_game_demo/prefabs/animation/player_policy.ron` | Player animation policy |
| `assets/projects/quick_scene/scenes/main.scene.ron` | Scene layout (player at y=4, giant at -10,0,0) |
| `assets/projects/quick_scene/assets.ron` | Both player and giant use `character-01.glb` |
| `bevy_ecs-0.18.0/src/hierarchy.rs` | Confirmed `Children` API for recursive search |
| `crates/ironhold_core/tests/integration_tests.rs` (lines 1409–1540) | Mock-GLTF tests — confirmed they use fake clips, not real GLB |

---

## Key architectural facts established

### Bevy 0.18 schedule order
```
First → PreUpdate → RunFixedMainLoop(FixedUpdate) → Update → SpawnScene → PostUpdate → Last
```
- `Assets<Gltf>` becomes available in **PreUpdate**.
- GLTF scene children are spawned in **SpawnScene** (after Update).
- Therefore `find_player_entity_recursive` always returns `None` on the **same frame** the GLTF first loads. Graph init is deferred to the next frame at minimum.

### AnimationGraphHandle + AnimationTransitions timing
Both are inserted via **deferred commands** on the frame graph init succeeds. They are not available until the **frame after** graph init. The playback system's `maybe_transitions = None` path handles this — it skips updating `last_played` so it retries next frame.

### `animation_policy_loader_system` timing
- Sets `controller.current = policy.base.idle` **immediately** (not deferred).
- Inserts `AnimationPolicyComponent` via **deferred command**.
- So the playback system can't run until the next frame (it requires `AnimationPolicyComponent` in its query).

### "No node index" permanent trap
In `animation_playback_system`, when `controller.current` is not in `node_indices`:
```rust
controller.last_played = controller.current.clone();
```
This sets `last_played = current` without playing anything. From this point forward, if `current` never changes, the system skips the playback block entirely (`current == last_played`). **Animations are permanently frozen.**

### `Action::PlayAnimation` broadcasts to ALL entities
```rust
Action::PlayAnimation(anim) => {
    for mut req in &mut animation_requests {
        req.queue.push_back(anim.clone());
    }
}
```
If a dance button sends `PlayAnimation("dance")`, the clip name `"dance"` is pushed to every entity with `AnimationRequests`. The resolver maps `"dance"` → the override clip for entities that have it, or tries to use it as a raw clip name for entities that don't. For the giant NPC, `"dance"` is a valid key in `clips` so it resolves correctly. But an unknown raw clip name that ends up in `controller.current` and is not in `node_indices` triggers the trap above.

### Giant NPC policy
```ron
// assets/projects/quick_scene/prefabs/animation/giant_npc_policy.ron
idle     = "Dance_Loop"
walk     = "Dance_Loop"
run      = "Dance_Loop"
jump_loop = "Dance_Loop"
clips: { "dance": "Dance_Loop" }
overrides: []
```
Only one unique clip name. The resolver should always land on `"Dance_Loop"`. T-pose means either:
- Graph never initialized (AnimationPlayer entity not found), OR
- `controller.current` got set to something not in `node_indices`, triggering the trap.

### `character-01.glb` confirmed clips (46 total)
All policy-referenced clips confirmed present:
`A_TPose`, `Dance_Loop`, `Idle_Loop`, `Jump_Land`, `Jump_Loop`, `Jump_Start`, `Sprint_Loop`, `Walk_Loop`, and 38 others.

---

## Bug found and fixed: `PendingPlayerConfig` entity leak

**File:** `runtime/scene_manager/scene_loader.rs` ~line 635  
**Problem:** When a scene has terrain, `spawn_scene_v2` spawns a `PendingPlayerConfig` entity to delay player creation until terrain is ready. This entity was spawned **without `LevelEntity`**, so the despawn sweep on scene transition (which despawns all `LevelEntity` entities) never cleaned it up.

On the second visit to any terrain scene, **two** `PendingPlayerConfig` entities exist → `spawn_player_when_terrain_ready` spawns **two players**. This explains "sometimes breaks" in `3rd_person_game_demo` when navigating back to menu and restarting.

**Fix applied (`scene_loader.rs:635`):**
```rust
// Before:
commands.spawn((
    crate::runtime::scene_manager::PendingPlayerConfig(pc),
    crate::runtime::scene_manager::PendingTonemapping(tonemapping),
));

// After:
commands.spawn((
    crate::runtime::scene_manager::PendingPlayerConfig(pc),
    crate::runtime::scene_manager::PendingTonemapping(tonemapping),
    LevelEntity,  // ← added; ensures cleanup on scene transition
));
```
Build confirmed clean.

---

## Diagnostic logging added: `capabilities/animation.rs`

Added `names: Query<&Name>` parameter to `animation_playback_system`. All log lines now include `[EntityName]` prefix. New/changed log points:

| Log level | Trigger | Message |
|---|---|---|
| `info!` | Graph init success | `[X] Animation graph ready: N clip(s) mapped, starting clip: "Y"` |
| `debug!` | Graph init deferred (hierarchy not ready) | `[X] Graph init deferred: AnimationPlayer not yet in hierarchy (GLTF: ...)` |
| `debug!` | `AnimationTransitions` not yet applied | `[X] Waiting for AnimationTransitions (deferred — retrying next frame)` |
| `warn!` | Clip not in `node_indices` | `[X] No node index for animation "Y" — available: [A, B, C]` |
| `warn!` | `AnimationPlayer` disappeared after graph init | `[X] AnimationPlayer entity lost after graph init — animation stalled` |

---

## Root cause of T-pose: NOT yet confirmed

Static analysis was inconclusive across multiple deep passes. The permanent trap is the mechanism, but for the 3rd_person_game_demo player, ALL clips resolved by the resolver ("jump_enter" → "Jump_Start", "jump_exit" → "Jump_Land", "dance" → "Dance_Loop", base locomotion clips) are known-good and present in character-01.glb. No code path was found that would push an unrecognised raw clip name to the player in normal gameplay.

Remaining candidates:
1. **Unknown `PlayAnimation` action**: some RON rule fires `PlayAnimation("some_unknown")` in an edge case not traced here. The resolver's raw-clip-name fallthrough would then put that string directly into `controller.current`.
2. **Intermittent Bevy scene hierarchy race**: `find_player_entity_recursive` returns an unexpected entity (e.g. a stale GLTF entity from a previous load), causing graph init to target the wrong `AnimationPlayer`. Bevy 0.18 `despawn()` is recursive so this is unlikely, but not disproven.
3. **Physics jitter causing double landing detection**: `was_grounded → is_grounded` transition fires twice in rapid succession, pushing `"jump_exit"` twice. Both pushes map to "Jump_Land" (valid), so this alone cannot cause T-pose.

---

## Defensive fixes applied (2026-05-04)

Two fixes were made to prevent the permanent trap from persisting regardless of the upstream cause:

### Fix 1 — `capabilities/animation_resolver.rs` step 4b

After choosing `chosen_clip`, validate it against `node_indices` once the graph is initialized. If the clip is missing, emit a `warn!`, clear the active override, and fall back to `policy.base.idle`. This prevents any bad clip name from ever being written to `controller.current`.

```rust
if anim_ctrl.graph_initialized
    && !anim_ctrl.node_indices.is_empty()
    && !anim_ctrl.node_indices.contains_key(&chosen_clip)
{
    warn!(...);
    active.clear();
    chosen_clip = policy.base.idle.clone();
}
```

### Fix 2 — `capabilities/animation.rs` "No node index" branch

Changed the permanent trap into a one-frame recovery: set `last_played = current` (silence per-frame spam) AND reset `current = policy_comp.0.base.idle`. On the next frame, `current ≠ last_played`, idle IS in `node_indices`, and the animation recovers.

```rust
controller.last_played = controller.current.clone();
controller.current = policy_comp.0.base.idle.clone();
```

### What the fixes guarantee
- The permanent trap can no longer freeze animations forever.
- If a bad clip is written (for any reason), the system self-heals within 1-2 frames.
- The root cause still emits a `WARN` log, making it identifiable if it ever fires.

---

## Root cause confirmed and fixed (2026-05-04)

The previous browser console showed correct graph initialization with 10 clips but NO WARN messages, yet the animation froze mid-air. The defensive fixes didn't help. Root cause still unknown.

**Root cause: Bevy's `SceneSpawner` re-spawns the GLTF scene mid-session.**

During the initial WASM load, sub-assets (textures, materials) arrive slightly after the scene. `SceneSpawner` first spawns the scene with placeholder data, then replaces the hierarchy when all dependencies resolve. The old `AnimationPlayer` entity (`126v0`) is despawned and a new one (`125v1`) is created. Our `AnimationGraphHandle` and `AnimationTransitions` were on the OLD entity. `graph_initialized` stayed `true`, so step 1 never ran for the new entity — permanent T-pose.

This is why the bug is much more frequent in WASM (HTTP fetch latency creates the re-spawn window) and on clean rebuilds (no browser cache for sub-assets). In native builds, all assets load from local disk in a single pass — the re-spawn window is so short it rarely triggers.

**Key insight that was blocking diagnosis:** the `debug!` "Waiting for AnimationTransitions" path is invisible in the web console at INFO level.

---

## Diagnostic logging added (2026-05-04)

Three new diagnostic warn/info points added in `animation.rs`:

| New message | What it means |
|---|---|
| `[X] Animation graph ready: N clips, starting clip: "Y", AnimationPlayer: Entity(Z)` | Extended to include entity ID of AnimationPlayer at init time |
| `[X] AnimationPlayer entity changed: A → B` | find_player_entity_recursive found a DIFFERENT entity than last time — node_indices are now stale |
| `[X] AnimationTransitions missing after "Y" was already played — player_ent Z` | Transitions existed when Y played, then disappeared — likely a second graph init on the same AnimationPlayer |
| `[X] player_query.get_mut(Z) returned Err` | AnimationPlayer entity found but not accessible via the query (despawn race or conflict) |

Also added: `controller.last_player_entity` stored at graph-init time (not just at first play).

---

## Fix applied (2026-05-04)

Added an **entity staleness check** between step 1 and step 2 in `animation_playback_system`. It runs every frame once `graph_initialized = true`, regardless of whether the current clip is changing.

If `find_player_entity_recursive` returns a different entity than `controller.last_player_entity`:
1. WARN with old and new entity IDs.
2. Reset `graph_initialized = false`, `last_player_entity = None`, `last_played = ""`.
3. `continue` — skip this frame.

Next frame: step 1 runs, finds the new entity, re-inserts `AnimationGraphHandle + AnimationTransitions`. 2-3 frames of T-pose during the re-spawn, then full recovery.

**Why the check must be outside `current != last_played`:** Static entities like NPCs that always play the same clip have `current == last_played` in steady state, so step 2 never runs. The entity change was silently ignored. Moving the check to a separate block before step 2 ensures all animated entities recover, not just ones whose animation changes frequently.

-- Notes Frank --
- Sometimes when I run the quick_scene project, the player character is T-posed. This is not always the case, but it happens frequently. I have not yet investigated this issue.

- recently (04-05-2026) the 3rd person game project has started to have the player character T-posed more often. The character is animated when it is created and then it falls from the spawn position and lands in a T-pose. At this point the animations stop working for the rest of the session. If the level is reloaded the problem sometimes goes away. I have a feeling that it might be a loading order issue. The issue occors more often when I do a clean rebuild. Or an issue with the animation policy, I am not sure which. The issue also uccorse mor eoften in web than native builds. This is also why I think it might be a loading order issue. I have not yet investigated this issue.