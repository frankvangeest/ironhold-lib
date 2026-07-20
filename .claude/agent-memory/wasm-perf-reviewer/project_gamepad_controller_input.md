---
name: project-gamepad-controller-input
description: General gamepad input feature — per-frame sorted-gamepad Vec pattern across 4 Update systems, RON-driven button parsing, WASM gilrs compat
metadata:
  type: project
---

General gamepad/controller input feature (branch `feature/gamepad-controller-input`, reviewed 2026-07-20). Touches `runtime/input.rs`, `capabilities/targeting.rs`, `capabilities/interactable.rs`, `capabilities/camera.rs`, `schema/player.rs`. Extends [[project_local_coop_input_camera]].

**Per-frame Vec pattern (accepted).** `input_translator_system` already built `Vec<(Entity, &Gamepad)>` collect+sort per frame; this feature adds the *same* pattern to 3 more Update systems (`tab_targeting_system`, `interactable_system`, `camera_orbit_system`) via new `pub(crate) resolve_gamepad(&sorted, index)` helper in input.rs. Player/gamepad count is 2-4, so collect+sort is ≤4 elements = tens of ns. Empty-gamepad case (common on desktop keyboard + WASM-before-pad-gesture) is alloc-free (empty query size_hint lower bound 0 → no heap alloc). Non-empty = one tiny short-lived Vec per system per frame; not worth SmallVec (would add a dep). Verdict: negligible.

**RON button parsing per frame (accepted, mirrors keyboard).** `InputMap::gamepad_button(name)` → `parse_gamepad_button(&str)` runs each frame per player — but this is exactly the existing accepted pattern: `key()`→`parse_key` and `tab_targeting_system`'s `InputMap::parse_key(&target_next)` already string-match every frame. No new regression; if ever optimized, do keyboard + gamepad together (pre-resolve at spawn like `look_*_key`).

**Spawn-time resolution done right for camera.** `OrbitCamera.gamepad_index` + `gamepad_deadzone` are pre-resolved at spawn (only the live stick *value* needs a per-frame query), mirroring the existing `look_*_key` spawn-resolution idiom. Right-stick-Y camera pitch is a net-new axis (only right-stick-X was used, for Turn).

**WASM/browser gamepad compat: OK.** `bevy_gilrs` already in tree (Bevy 0.18, default features), gilrs-core has a wasm32 backend over the browser Gamepad API. Known browser behaviors (all expected, not bugs): Gamepad API is polling-based (`navigator.getGamepads()`, polled each frame — matches Update scheduling); a pad is invisible until the user presses a button on it (privacy gesture requirement) so it won't appear at scene load; assumes "standard mapping" layout. No compute/WebGL2 concerns — pure input.

**Binary size: zero impact.** No new deps (empty Cargo.lock/toml diff). New `parse_gamepad_button` match (16 arms) + 5 `InputMap` String/f32 fields are trivial pure-data additions.

**Bonus bug fix folded in:** `interactable_system` converted from `player_query.single()` to per-player loop (single() silently no-op'd interact for ALL players in any 2+ CharacterController scene). Predates gamepad feature; fixed here.
