# Feature: Per-Viewport-Only Target Ring Visibility

_Status: Ready_
_Planned at: `af55a1b` (2026-07-19)_

**Plan-review note (2026-07-19):** All three reviewers (system-architect, ux-gamedesigner-reviewer,
wasm-perf-reviewer) independently caught the same critical defect in the first draft: leaving the
shared `PartyOrbitCamera` with **no** `RenderLayers` component (implicit layer {0}) while every
ring in `OwnViewportOnly` mode carries **only** its own reserved layer means layer {0} ∩ {1+idx} =
∅ — the party/merged camera would render **zero** rings, directly contradicting the plan's own
"merged view shows all rings" acceptance criterion. Fixed: the party camera now gets an explicit
`RenderLayers::from_layers(&[0, 1, 2, 3, 4])` union (layer 0 + every reserved ring layer) when the
mode is `OwnViewportOnly`, applied in `spawn_party_orbit_camera`. **system-architect** additionally
found the camera-insertion task only named one of the two split-camera spawn loops (the static
branch, `entity_spawner.rs:706-721`) — the `dynamic`-split branch (`:665-675`) needed it too, since
the plan's own scope explicitly includes `dynamic` — and that both loops must key the camera's
layer on `config.player_index` (the same field the ring keys on), not the loop's spawn-order index
`i`, to avoid a camera/ring layer mismatch whenever `player_index` isn't scene order. Also added:
`init_resource::<TargetRingVisibilityMode>()` (the resource must exist for every scene, including
single-player/party-only, not just scenes that hit the split branch) and a note that the camera
loops read `split.ring_visibility` directly (already in scope) rather than reading back the
resource they just wrote via deferred `Commands`. **ux-gamedesigner-reviewer** changed the field
from a two-variant enum to `own_viewport_only: bool`, matching this codebase's overwhelming house
style for binary opt-ins (`allow_manual_zoom`, `cast_shadows`, etc. — enums here are reserved for
genuinely multi-state axes); required the demo to be a live-authored new scene rather than a
commented-out field, since the acceptance criteria are browser-observable and a commented field
can't be verified in the shipped build; added a doc task noting `PLAYER_LABEL_COLORS` tinting is
gated on player *count*, not this field, so per-target `indicator_color` still won't apply in
`OwnViewportOnly` mode — the single most likely support question this feature generates; and moved
the reserved-layer-index detail out of the designer-facing `docs/20` into the engine-internal
`CLAUDE.md`. **wasm-perf-reviewer** confirmed the approach is spawn-time-only with zero per-frame
cost and no WASM/WebGL2-vs-WebGPU gap, but flagged that `pipeline_warmup_system`'s
`NoFrustumCulling` warmup pass does not override `RenderLayers` — benign here since rings reuse an
already-warm material, but worth documenting as an invariant — and added a `test_web.py`/browser
verification task.

## What
Adds an opt-in `SplitScreenDef.own_viewport_only: bool` (default `false`, today's behavior) so a
player's target-indicator ring is visible **only in their own viewport**, instead of every ring
being visible in every viewport (today's default, disambiguated by per-player tint).

## Why
Today (per `per_player_split_screen_targeting.md` Phase 1) every player's ring renders in every
active split viewport, tinted via `PLAYER_LABEL_COLORS` so whose ring is whose stays clear. Some
designers may instead want a player to only ever see their *own* ring — surfaced 2026-07-15 as a
follow-up during that feature's playtest review, not implemented, no plan file until now.

## Approach

**New schema field** on `SplitScreenDef` (`schema/player.rs`, the `camera.split:` block already
read only from the first player-tagged scene entity, matching every other split-screen switch's
existing convention): `#[serde(default)] pub own_viewport_only: bool` — matching this codebase's
house style for binary opt-ins (`allow_manual_zoom`, `cast_shadows`, etc.) rather than a two-variant
enum for what is genuinely a single toggle.

**Bevy `RenderLayers`, applied for the first time to a designer-facing feature** (the only existing
usage in this crate is `inspector.rs`'s debug camera, a pure engine-internal isolation concern, and
is `#[cfg(feature = "inspector")]`-gated out of normal builds entirely — this establishes the
convention from scratch). A camera only renders entities whose layers intersect its own; every
existing entity and camera has no `RenderLayers` component and so implicitly sits on layer 0 — every
layer assignment below deliberately accounts for this rather than assuming isolation is free.

**Reserved layer scheme** (engine-internal, documented in `crates/ironhold_core/src/CLAUDE.md`, not
`docs/20` — a designer never authors a layer index directly): layers **1-4** reserved for
per-split-player ring visibility, indexed identically to `PLAYER_LABEL_COLORS`' own scheme
(`1 + player_index.0 % MAX_SPLIT_PLAYERS`, same modulo-collision behavior that palette already has
— acceptable, unchanged). Layer 31 (inspector, feature-gated) is untouched.

**Only when `own_viewport_only == true`** (zero component footprint otherwise — no `RenderLayers`
inserted anywhere for the default, fully backward compatible):
- Each split `OrbitCamera` — spawned in **both** of `spawn_players_and_camera`'s split-camera
  loops (`entity_spawner.rs:665-675`, the `dynamic` branch, **and** `:706-721`, the static
  `Grid`/`Vertical`/`Horizontal` branch — both need this insertion, not just the static one) — gets
  `RenderLayers::layer(0).with(1 + config.player_index % MAX_SPLIT_PLAYERS)`, keyed on
  **`config.player_index`, not the loop's spawn-order index `i`** (they can diverge; the ring is
  keyed on `PlayerIndex.0`, so the camera must match that exact value, not scene-entity order).
- Each ring entity (`target_indicator_system`'s spawn site, `target_indicator.rs:169-177`) gets
  `RenderLayers::layer(1 + owner_player_index % MAX_SPLIT_PLAYERS)` **only** — invisible to every
  camera except the one matching its owning player's layer.
- The shared `PartyOrbitCamera` (party mode, and `dynamic` split's merged state) gets an explicit
  `RenderLayers::from_layers(&[0, 1, 2, 3, 4])` — layer 0 (ordinary scene geometry) **plus every
  reserved ring layer**, so it still sees all rings when it's the one active camera. This is the
  corrected version of the original (wrong) "no component at all" design — see the plan-review note.
  Since `dynamic_split_screen_system` never spawns/despawns cameras (only toggles `is_active`,
  confirmed against source), this composes correctly with zero per-frame layer changes: whichever
  camera is currently rendering already has the right static layer set from spawn time.

**Scope**: applies to `Grid`/`Vertical`/`Horizontal`/`dynamic` split (any mode with real per-player
`OrbitCamera`s). `party`-only scenes (no split cameras at all) have no per-viewport concept to
restrict to — the field is simply inert there; a load-time `warn!` when `own_viewport_only: true`
is authored on a scene with no split cameras at all is a nice-to-have (matches this codebase's
warn-on-contradictory-authored-intent principle) but not required for v1.

**New resource**: `TargetRingVisibilityMode`, `init_resource`'d (default `AllViewports`) so
`target_indicator_system` never hits a missing-resource panic in single-player/party-only scenes —
overwritten (not just conditionally inserted) alongside `ActiveSplitScreen`/`DynamicSplitConfig` at
both `spawn_players_and_camera` call sites. The two camera-spawn loops read `split.ring_visibility`
directly (already in scope as a local variable) rather than reading back the resource they
themselves just wrote via deferred `Commands` in the same system.

**Designer-facing interaction to document explicitly**: `own_viewport_only: true` does **not**
restore per-target ring color (`indicator_color`/`indicator_category`/scene `color` precedence) —
that precedence is already overridden by `PLAYER_LABEL_COLORS` tinting whenever 2+ players are
present (gated on player *count*, not on this field), so a ring still shows its owning player's
palette color even when it's the only ring that player can see. Keeping the tint is the right
default (harmless, and still meaningful for a spectator/co-located view), but this is the single
most likely "why isn't my ring color working" question this feature will generate if undocumented.

## Tasks
- [ ] `SplitScreenDef.own_viewport_only: bool` (schema/player.rs, `#[serde(default)]`)
- [ ] `TargetRingVisibilityMode` resource; `init_resource`'d at app startup (default
      `AllViewports`), overwritten at both `spawn_players_and_camera` call sites alongside
      `ActiveSplitScreen`/`DynamicSplitConfig`
- [ ] `entity_spawner.rs`: insert `RenderLayers::layer(0).with(1 + config.player_index %
      MAX_SPLIT_PLAYERS)` on each split `OrbitCamera` in **both** spawn loops (`:665-675` dynamic,
      `:706-721` static), keyed on `config.player_index` — only when `split.own_viewport_only` is
      true, read directly from the in-scope `split: &SplitScreenDef`, not the resource
- [ ] `entity_spawner.rs`'s `spawn_party_orbit_camera` helper: insert `RenderLayers::from_layers(&[0,
      1, 2, 3, 4])` on the party camera when `own_viewport_only` is true (the corrected union, not
      "no component")
- [ ] `target_indicator_system`: insert `RenderLayers::layer(1 + owner_player_index %
      MAX_SPLIT_PLAYERS)` on the ring entity only when `TargetRingVisibilityMode == OwnViewportOnly`
      — reuses the `player_index` lookup already present at this spawn site for color tinting
- [ ] Explicit `use bevy::camera::visibility::RenderLayers` import at both call sites (not in
      `bevy::prelude`, and both files currently glob-import the prelude)
- [ ] Tests — regression: a scene with no `own_viewport_only` authored (every existing project)
      spawns zero `RenderLayers` components anywhere, byte-for-byte unchanged; new: in
      `own_viewport_only: true` mode, player 0's ring carries only layer 1, player 0's camera
      carries `{0,1}`, player 1's camera carries `{0,2}` (no intersection with player 0's ring);
      new: the party/merged camera carries the full `{0,1,2,3,4}` union and therefore intersects
      every ring's layer — corrected from the original (wrong) "no component" expectation
- [ ] Verify in a browser (`python test_web.py` and/or manual playtest on the WebGL2 build) that
      per-viewport ring visibility actually renders as expected — `RenderLayers` behaves identically
      on WebGL2/WebGPU per this session's review, but this is the feature's first real-render check
- [ ] Docs — `docs/20_data_formats.md`: `SplitScreenDef.own_viewport_only` field (observable
      behavior only — own-viewport vs. all-viewports — no layer-index internals); a cross-reference
      from the `target_indicator:` doc section to this field, since a designer tuning ring
      appearance in one block won't naturally discover a visibility toggle living in `camera.split`;
      the tint-does-not-revert-to-per-target-color note (see Approach) as its own explicit bullet
- [ ] Docs — `crates/ironhold_core/src/CLAUDE.md`: the reserved-layer-index scheme (1-4 this
      feature, 31 inspector/feature-gated), the party-camera-union rationale, and a note that
      `pipeline_warmup_system`'s `NoFrustumCulling` pass does not override `RenderLayers` (benign
      here since rings reuse an already-warm material, but a future `RenderLayers` consumer should
      know this)
- [ ] Demo — add a **new**, live-authored `local_coop_demo` scene (a sibling copy of `room3`, the
      existing per-player-targeting playtest room, with `own_viewport_only: true` set on the first
      player's `camera.split` block) rather than mutating `room3` in place (which demonstrates the
      *default* all-viewports behavior and should keep doing so) or using a commented-out field
      (unobservable in the shipped build, contradicting the browser-observable acceptance criteria)

## Open questions
None outstanding.

## Acceptance criteria
- Given the new `local_coop_demo` room with `own_viewport_only: true` authored, when P1 selects a
  target, then P1's ring is visible in P1's own viewport only — it does not appear in P2's viewport
  (**browser-observable**).
- Given the same scene, when P2 selects a different target, then P2's ring is likewise confined to
  P2's own viewport, and P1's ring is unaffected.
- Given any pre-existing split-screen scene with no `own_viewport_only` authored, when this ships,
  then every ring still renders in every viewport exactly as today — regression, not just a passing
  test.
- Given a `dynamic` split scene in its merged (single-viewport, party-camera-active) state, when
  this ships, then all players' rings are visible in that single shared view regardless of
  `own_viewport_only` — the restriction only applies while actually split into separate viewports.
- Given `own_viewport_only: true`, when a ring renders, then it is still tinted by
  `PLAYER_LABEL_COLORS` (not per-target `indicator_color`) — documented, not a bug.
