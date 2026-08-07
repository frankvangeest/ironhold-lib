---
name: unregistered-systems-silent
description: New pub fn systems in capabilities/ produce no dead-code warning if never added to lib.rs's schedule — always grep lib.rs for each new system name
metadata:
  type: project
---

A newly written `pub fn *_system(...)` inside a `pub mod` under `capabilities/` is part of
the crate's public API, so Rust emits **no** `dead_code` warning when it is never added to
an `app.add_systems(...)` call. The full test suite can be green and the WASM build clean
while the system never runs.

**Why:** observed at the camera_modes v1 review (2026-08-07) — `follow_camera_system`,
`first_person_camera_system` and `fixed_camera_system` were fully written, documented and
shipped in the docs as implemented, but appeared nowhere in `crates/ironhold_core/src/lib.rs`.
Integration tests that call `app.update()` on a hand-built `App` also miss this, because
they register the systems under test explicitly.

**How to apply:** on any review that adds a new Bevy system, run
`grep -rn "<system_name>" crates/ --include=*.rs` and confirm at least one hit in `lib.rs`
(or the relevant plugin `build()`), plus a sane `.chain()`/`.after()` position. Cheap check,
catches a whole feature being inert.
