# Multi-Source Animations (Animation Packs + Shared-Rig Mesh Variants)

Planned at: `423d5a7` (2026-06-08)

---

## Problem

Every character's `AnimationController` builds its clip graph exclusively from the clips
embedded in the model GLB (`PrefabDef.model`).  This forces all animations for a character
to live in one file, which creates two practical blockers:

1. **Split animation authoring is impossible.** Studio pipelines often produce per-domain
   animation files: `basic_locomotion.glb`, `magic_spells.glb`, `gun_stances.glb`.
   Each is exported separately and must currently be collapsed into one mega-GLB before
   ironhold can use them.

2. **Mesh variants share no animation work.** A `player_male` and `player_female` that share
   an identical skeleton must each carry a full copy of every clip in their model GLBs, or
   the designer has to maintain two identical `AnimationPolicy` files pointing to duplicate
   GLBs.

---

## Current system (relevant parts)

```
PrefabDef.model              →  catalog key → GLB path
PrefabDef.animation_policy   →  path to AnimationPolicy.ron

AnimationController {
    gltf_path: String,        // the model GLB path
    gltf_handle: Handle<Gltf>,
    node_indices: HashMap<String, AnimationNodeIndex>,
    graph_initialized: bool,
}

// Graph init (animation.rs):
//   wait until gltf_handle is ready
//   for each clip_name in policy → look up gltf.named_animations[clip_name]
//   add matching clips to AnimationGraph, warn on missing
```

`AnimationPolicy.ron` (current schema):
```ron
(
    default_transition_ms: 150,
    base: ( idle: "Idle_Loop", walk: "Walk_Loop", run: "Sprint_Loop", jump_loop: "Jump_Loop" ),
    clips: { "attack": "Sword_Attack" },
    overrides: [ ... ],
)
```

---

## Proposed design — v1

### Schema change: `animation_sources` on `AnimationPolicy`

Add an optional list of **additional** GLB catalog keys whose `named_animations` are merged
into the character's animation graph at startup:

```ron
// magic_player_policy.ron
(
    default_transition_ms: 150,
    animation_sources: ["basic_locomotion", "magic_spells", "gun_stances"],
    base: ( idle: "Idle_Loop", walk: "Walk_Loop", run: "Sprint_Loop", jump_loop: "Jump_Loop" ),
    clips: {
        "fireball":     "Cast_Fireball",
        "reload":       "Gun_Reload",
        "aim_idle":     "Gun_Aim_Idle",
        "attack_light": "Sword_Attack",
    },
    overrides: [ ... ],
)
```

The model GLB (`PrefabDef.model`) continues to supply the mesh and skeleton.  It no longer
needs to contain any animation clips — the `animation_sources` GLBs provide them.  Clips
embedded in the model GLB are still included if present (backwards-compatible).

**Merge rule:** clip names are unique within a character's graph.  If two sources export a
clip with the same name, the later entry in `animation_sources` wins and a `warn!` is emitted.

### Runtime change: `AnimationController` multi-handle loading

```rust
pub struct AnimationController {
    // existing fields unchanged
    pub gltf_handle: Handle<Gltf>,        // model GLB (mesh + skeleton)
    // NEW
    pub source_handles: Vec<Handle<Gltf>>, // extra animation-pack GLBs
}
```

Graph initialisation waits until `gltf_handle` **and** all `source_handles` are
`Assets<Gltf>::get(handle) = Some(_)` before building the merged `AnimationGraph`.
The existing "defer one frame if not ready" retry loop already handles the timing.

### Mesh variants (male / female using the same animation pack)

No new schema is required for v1.  The pattern is:

```ron
// prefabs.ron
(prefabs: {
    "player_male": (
        kind: Actor,
        model: "character_male",        // mesh GLB — compatible skeleton, no clips needed
        animation_policy: "prefabs/animation/player_policy.ron",
    ),
    "player_female": (
        kind: Actor,
        model: "character_female",      // different mesh, same skeleton
        animation_policy: "prefabs/animation/player_policy.ron",  // SHARED policy
    ),
})
```

Both prefabs reference the same `player_policy.ron`, which declares
`animation_sources: ["basic_locomotion", "magic_spells"]`.  The mesh GLBs carry the
skeleton (armature) but no clips; all clips come from the shared animation packs.

Constraint (designer responsibility): all GLBs — model and sources — **must use the same
bone names** (i.e. share a common rig).  Bevy resolves `AnimationClip` targets by
`EntityPath` (bone name chain); mismatched names silently produce no movement with a `warn!`.

---

## Out of scope for v1

- **Runtime skin/costume swap** — swapping `character_male.glb` for `character_female.glb`
  on a live entity without a scene reload.  This is a separate, more invasive feature
  (requires despawning/respawning the `SceneRoot` child while preserving the animation
  graph state).  Park in Icebox.
- **Bevy animation retargeting** — for rigs that are *close but not identical* (different
  bone count / naming conventions), Bevy 0.15 introduced retargeting.  Evaluate if
  bone-name matching proves insufficient in practice.
- **Hot-reload of animation sources** — follow-up once general hot-reload ships.

---

## Implementation plan

1. **`AnimationPolicy` schema** (`schema/catalog.rs` or wherever `AnimationPolicy` is defined)
   - Add `#[serde(default)] pub animation_sources: Vec<String>` (default = empty vec).
   - No schema version bump (purely additive field).

2. **`AnimationController` struct** (`capabilities/animation.rs`)
   - Add `pub source_handles: Vec<Handle<Gltf>>`.

3. **Spawner wiring** (`runtime/scene_manager/scene_loader.rs` or wherever `AnimationController` is inserted)
   - After resolving `animation_policy`, load each `animation_sources` key from the catalog,
     push handles into `source_handles`.

4. **Graph init** (`animation_playback_system`)
   - Before `graph_initialized = true`, check that all `source_handles` are loaded.
   - Collect `named_animations` from all source GLBs; merge into one map (last-wins on dup, warn).
   - Build graph from merged map exactly as today.

5. **Tests** — integration test: prefab with `model` containing no clips + one `animation_sources`
   GLB; verify clips are found and graph initialises.

6. **Docs** — `docs/20_data_formats.md` AnimationPolicy section; add `animation_sources` field
   description and the shared-rig mesh variant pattern.

---

## Open questions

- Should `animation_sources` keys be required to exist in `assets.ron`?  Probably yes — same
  validation rule as `model`; fail loudly at `validate` time if a key is missing.
- Do we also add `animation_sources` support to the `ironhold_cli query prefabs` output?
  Low value for v1 — skip.
