---
name: update-globaltransform-one-tick-stale
description: Any Update-schedule system reading GlobalTransform of a physics body sees a value one FixedUpdate tick behind what renders; camera systems read the fresh Transform instead, so the skew is invisible on meshes and visible on world-space UI
metadata:
  type: project
---

Once Rapier runs `in_fixed_schedule()` (landed with `deterministic_fixed_timestep` v1), the frame
order is `FixedUpdate` (N times) → `Update` → `PostUpdate`. Within a FixedUpdate tick, Rapier's
`SyncBackend` runs `sync_simple_transforms` + `propagate_parent_transforms` **before** the step;
`Writeback` (`writeback_rigid_bodies`, `rigid_body.rs:403`) writes **only `Transform`**, never
`GlobalTransform`, and nothing propagates after it. So:

- **`GlobalTransform` read in `Update` = the entity's pose one physics tick before what renders**
  (render uses the PostUpdate-propagated `Transform`).
- **`Transform` read in `Update` = current.** `camera_orbit_system` (`camera.rs:196`) reads
  `&Transform` of the character, so camera + mesh are locked to the same sample — the player mesh
  stays pixel-stable no matter how many ticks ran. That asymmetry is why a cadence bug shows up on
  world-space UI and *not* on character motion.

Known `Update` readers of `GlobalTransform` on physics-driven entities:
`world_label_screen_pos_system` (`lib.rs:612`, nameplates/stat bars/damage popups),
`target_indicator_system` (`target_indicator.rs:83`, the ring), `decal.rs:107`,
`targeting.rs:263/265/335`, `fixed_camera_system` (`camera.rs:495`). The first three *write a
Transform derived from it*, so they visibly lag; the rest only do distance math and don't care.

**The trap:** the label system reads a stale trackee GT *and* a stale camera GT, so at 0 or 1 tick
per frame the two staleness errors cancel exactly and nothing is visible. Only a **2-tick frame**
breaks the cancellation (trackee advances one tick, camera sample doesn't) → a one-frame pop of
one tick of trackee motion. Consequence: **"just propagate transforms after `PhysicsSet::Writeback`"
makes it worse, not better** — it freshens the trackee while leaving the camera stale, turning a
0/0/1-tick error into 0/1/2. Any fix must sample camera and trackee at the *same* instant.

**How to apply:** when a world-space-UI stutter is reported and character motion is smooth, this is
the first thing to check, and the discriminator is the trackee's own speed (stand still + orbit the
camera → no pop; run → pop; walking NPC's plate pops proportional to its walk speed). Related:
[[fixedupdate-vs-rapier-clock]], [[per-frame-changedetection-transform-writes]],
[[vsync-defeats-framepace-tick-matching]].
