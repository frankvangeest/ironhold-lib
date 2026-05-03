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

Static analysis was inconclusive. The permanent trap is the mechanism, but what pushes the bad clip name into `controller.current` is still unknown. Candidates:

1. **Timing race**: graph init runs, `controller.current` already changed to something before `node_indices` is populated. Unlikely given the deferred-command ordering.
2. **Unknown override push**: `player.rs` pushes `"jump_enter"` / `"jump_exit"` as override IDs. Both exist in the player policy's overrides list. But if the override resolver sees a name it doesn't recognize, it falls through to raw clip name, which then fails.
3. **`Action::PlayAnimation` with a raw string**: if a RON rule fires `PlayAnimation("some_name")` and that string is not an override key or a `clips` key, the resolver may set `controller.current` to it directly — and if it's not in `node_indices`, the trap fires.
4. **Giant NPC specific**: the graph might never initialize if `find_player_entity_recursive` fails due to scale (3,3,3) or entity hierarchy differences. The new `debug!` logs will confirm or rule this out.

---

## Next steps to complete the investigation

1. **Run the game and capture logs:**
   ```bash
   cargo run -p ironhold_native -- --project 3rd_person_game_demo 2>&1 | tee /tmp/anim_log.txt
   ```
   Reproduce the T-pose (spawn → fall → land), then:
   ```bash
   grep -E "WARN|INFO.*Animation graph|No node index" /tmp/anim_log.txt
   ```

2. **Look for:**
   - `[PlayerName] No node index for animation "X"` — the value of X is the root cause.
   - `[PlayerName] Animation graph ready: N clip(s)` — verify N > 0 and the starting clip is correct.
   - `[GiantName] Graph init deferred` repeating every frame — means `AnimationPlayer` never found in hierarchy.

3. **For the giant NPC** — also run `quick_scene` and check if graph ever initializes:
   ```bash
   cargo run -p ironhold_native -- --project quick_scene 2>&1 | grep -E "giant|Giant|Animation graph"
   ```

4. **Once root cause confirmed**, fix the upstream push of the bad clip name (either in `animation_resolver.rs`, `player.rs`, or the RON rules file).

-- Notes --
Sometimes when I run the quick_scene project, the player character is T-posed. This is not always the case, but it happens frequently. I have not yet investigated this issue.