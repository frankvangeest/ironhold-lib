---
name: gamepad-binding-pattern
description: BoundGamepad seed-then-lock model, the 4 stale schema doc-comment sites it left behind, and the per-scene-not-per-catalog scoping rule for player-prefab validation checks
metadata:
  type: project
---

**`gamepad_player_binding_hardening.md` (reviewed 2026-08-01, ALIGNED w/ doc warnings)** replaced
live positional `resolve_gamepad(sorted_slice, gamepad_index)` (deleted) with
`BoundGamepad(pub Option<Entity>)` (`capabilities/player.rs`), resolved once by
`gamepad_bind_system` (`runtime/input.rs`, FixedUpdate `.chain()` before `input_translator_system`).
`InputMap.gamepad_index` is now a **one-time seed**, never a live index. Zero RON surface added or
removed — the whole change is behind one unchanged designer field.

**Five gamepad consumers all read `bound.and_then(|b| b.0).and_then(|e| gp_q.get(e).ok())`:**
`input_translator_system`, `tab_targeting_system`, `interactable_system`, `action_bar_input_system`
(all take `Option<&BoundGamepad>` — tolerant), and `camera_orbit_system` (resolves through
`orbit.target` via `bound_q: Query<&BoundGamepad>`; `OrbitCamera.gamepad_index` was **deleted** to
kill the spawn-frozen second source of truth — `gamepad_deadzone` stays, it's real tuning).
`gamepad_bind_system` is the sole writer and the only one with a **required** `&PlayerIndex` — safe
today because `CharacterController { .. }` is constructed at exactly ONE site
(`entity_spawner.rs::spawn_player_entity_core`, ~line 948) which inserts `PlayerIndex` +
`BoundGamepad` in the same block. Grep `CharacterController {` before assuming that still holds.

**`PlayerConfig.bound_gamepad: Option<Entity>` is a legal pure-runtime field** — `PlayerConfig`
carries an explicit comment that it is deliberately NOT `Deserialize` (assembled by
`assemble_player_config`), so adding non-RON fields there is fine and not a schema leak. Set to
`None` by `assemble_player_config`; only `Action::JoinPlayer` sets it (`.take()` from
`PendingJoinGamepad`), closing the old `Entity → index → Entity` hot-join round-trip.

**RECURRING FOOTGUN — schema/RON doc comments lag behind gamepad semantics.** `docs/20_data_formats.md`
was updated thoroughly, but four in-code designer-facing comments were left stale by this change:
`schema/player.rs` `InputMap.gamepad_index` ("reads input ... instead of the keyboard" — wrong on
BOTH additivity and seed-vs-live), `schema/scene_v2.rs` `ActionSlotDef.gamepad_key`
("owner_player -> that player's `InputMap.gamepad_index`"), `schema/project.rs`
`global_unclaimed_gamepad_bindings` ("not bound to any live player's `InputMap.gamepad_index`" — the
claimed set is `HashSet<Entity>` from `BoundGamepad` now), and `local_coop_demo/scenes/room8.scene.ron`
("joining via gamepad only sets that player's InputMap.gamepad_index"). Always grep
`gamepad_index` across `crates/*/src/schema/` AND `assets/projects/**/*.ron` comments on any gamepad
change — `docs/` alone is not the whole designer-facing surface. **Update: all four fixed during
this feature's own post-implementation-review pass (2026-08-05).**

**Per-scene-not-per-catalog is the correct scoping for any player-prefab cross-check.**
`local_coop_demo`'s catalog legitimately reuses `gamepad_index: 0/1` across room variants
(`player_p1_split` vs `player_p1_split_ring`, prefabs.ron ~456/~1202) that are never co-instantiated
— a catalog-wide check false-positives and breaks `cargo test -p ironhold_cli --test
validate_projects`. Both `scene_loader.rs::warn_duplicate_gamepad_index` (iterates the assembled
`player_configs`) and the CLI `duplicate_gamepad_index` check (iterates `scene.entities` →
`catalog.prefabs`) get this right; `validate_local_coop_demo` is the standing negative-case test.

**New `ironhold_cli validate` error types must also be named in `docs/20_data_formats.md`** — the
sibling `gamepad_key_without_gamepad_index` is documented inline in the `gamepad_key` note
(~line 1001); `duplicate_gamepad_index` shipped undocumented at first review, then documented in the
same post-review pass. A hard validate error with no doc reference is a designer dead end.

**Accepted non-RON constants here:** `GAMEPAD_DIAGNOSTIC_WARN_SECS = 3.0` (log-only diagnostic
threshold, same class as `SPAWNS_PER_FRAME`). **Known accepted RON gap** (explicit in the plan's
"out of scope"): "pending" and "bound-but-disconnected" are now well-defined engine states whose
only surface is a Rust `warn!` — no `GameVariable`/`GameEvent`, so a designer cannot author a
"Controller disconnected" banner (logged to `planning/backlog.md`'s Icebox).

**Post-review-fix additions (2026-08-05), not seen by the original review pass:**
- `gamepad_bind_system`'s `claimed` set now also chains in undrained `is_hot_join`
  `PendingEntitySpawns` entries' `bound_gamepad` — mirrors `unclaimed_gamepad_trigger_system`'s
  equivalent chain, closing a hole where a pending scene player could bind the same pad an
  in-flight hot-join spawn had already captured.
- `unclaimed_gamepad_trigger_system` now also reserves the pad a still-pending live player's own
  seed resolves to, since `gamepad_bind_system` (`FixedUpdate`) can lag a frame behind it (`Update`).
- Real-hardware playtest found a controller that reliably registered as two browser gamepad
  entries; added `GAMEPAD_STABLE_CONNECT_SECS = 0.5` — a candidate pad must be continuously present
  this long before `gamepad_bind_system` will commit a binding to it, so a same-session spurious
  duplicate entry can vanish before ever being locked onto.

See [[local_coop_pattern]] for the surrounding co-op spawn/camera model.
