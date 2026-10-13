# Feature: Pre-release render fixes (ambient light, warmup NoFrustumCulling, canvas pixel cap)

_Status: Draft (plan-review pending)_
_Planned at: `d4ab4e0` (2026-10-12)_

Source: `planning/stakeholder_priority_list.md` (3rd edition) — Game-World-Designer #2, WASM-Perf #1 and #5,
verified by the debug-detective pass (annotations in that file). Three small, independent fixes batched on one branch
because they are all tiny, all touch the render/lighting boot path, and all should land before the next `/release`.

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| A | Bug: authored scene ambient light does nothing | Queued | — |
| B | Bug: `pipeline_warmup_system` strips `NoFrustumCulling` from entities it did not add | Queued | — |
| C | Perf: cap the fitted canvas's total physical pixels | Queued | — |

Each phase is its own commit on the feature branch and is independently revertable. **Single feature file, three
backlog items:** the file moves to `done/` only when all three ship.

## What

- **A.** `lighting: ( ambient: .., ambient_brightness: .. )` in a scene RON starts working. Today it is documented
  (`docs/20` ~L286) but has no effect.
- **B.** Entities that rely on a permanent `NoFrustumCulling` (the particle pool meshes today, the ocean grid later) keep
  it after the 4-frame pipeline warmup.
- **C.** The web canvas can no longer grow past a fixed physical pixel budget, however large the monitor or DPR.

## Why

All three are broken or regressing *now*, are confirmed from source (A, B) or arithmetic (C), and each blocks later work
(A blocks day/night and the ocean `EnvState`; B blocks the ocean grid; C ships with the next release and costs ~3.9×
fill-rate on large monitors). See the backlog Bugs entries and the Performance entry.

## Approach

### A — ambient light

`apply_lighting_v2` (`scene_loader.rs` ~L2998) spawns an `AmbientLight` *entity*. In Bevy 0.18 `AmbientLight` is a
per-camera override (`#[require(Camera)]`, `bevy_light-0.18.0/src/ambient_light.rs:8-12`); scene-wide ambient is the
`GlobalAmbientLight` **resource** (`bevy_pbr-0.18.0/src/render/light.rs:705-723`, fallback at `:1153-1158`).

- Replace the entity spawn with `commands.insert_resource(GlobalAmbientLight { color, brightness, ..default() })`.
- `apply_lighting_v2` is called only from the Replace branch (overlays never reach it), so the resource can be set on
  every base-scene load. **When a scene has no `ambient:`, reset the resource to `GlobalAmbientLight::default()`**, so an
  authored ambient does not leak into the next scene (the old entity was `LevelEntity` and so was torn down per scene;
  a resource is not).
- No schema change: `SceneLightingV2.ambient` / `ambient_brightness` keep their names and the `150.0` default.
  Fix the `schema/scene_v2.rs` doc comment ("`AmbientLight::brightness`" → `GlobalAmbientLight`).
- **Visual side effect (needs a decision before merging):** 37 scenes set `lighting.ambient`, so their look will change
  (authored colour and brightness now apply; they were rendered with Bevy's default white/80 cd/m² while being tuned).
  Brightness default is 150 vs Bevy's 80. Affected project screenshot baselines must be re-captured
  (`--real-gpu`, only the affected projects, own commit). Alternative considered and rejected: silently scaling to keep
  the old look — it would make the documented values lie.
- Also removes the stray render-graph-less `Camera` entity the old code spawned (bevy_render warn on every such load).

### B — warmup removes only what it added

`pipeline_warmup_system` (`lib.rs:480-503`) inserts `NoFrustumCulling` on every `Mesh3d` for 4 frames, then removes it
from **every** `With<NoFrustumCulling>` entity. `particle_renderer.rs:364/392` spawn pool meshes with a *permanent*
`NoFrustumCulling` (world-space vertices, identity transform), so a pool created inside the window is un-marked and then
culled against a stale AABB.

- Add a private marker component `WarmupNoCull`. While counting down, insert `(NoFrustumCulling, WarmupNoCull)` only on
  `Mesh3d` entities that **do not already have** `NoFrustumCulling`. At zero, remove `NoFrustumCulling` and
  `WarmupNoCull` only from `With<WarmupNoCull>` entities.
- Entities that already carry a permanent `NoFrustumCulling` are never touched. `PipelineWarmup(4)` re-arm per scene
  load is unchanged.
- Update the `capabilities/CLAUDE.md` and `src/CLAUDE.md` warmup sentences, and the `ocean_simulation_demo.md` B1/N17
  notes (the ocean grid can now rely on permanent NFC).

### C — canvas pixel cap (host page, not engine)

Research pass (pinned sources, no browser): the canvas `width`/`height` attributes are written by **wgpu** on every surface
configure (`wgpu-27.0.1/src/backend/webgpu.rs:3833-3838`), from the physical size winit's ResizeObserver reports (the
canvas CSS content box × `devicePixelRatio`; ResizeObserver content boxes ignore CSS transforms). Consequences:

- `WindowResolution::scale_factor_override` does **not** cap pixels (it only changes Bevy's logical size and
  `scale_factor()`, writes explicit CSS px that fights `fit_canvas_to_parent`, and would break the `Val::Px` consumers that
  divide by `scale_factor`, `camera.rs:864/959`). Rejected.
- No usable camera-level render scale exists in Bevy 0.18 (`Camera.viewport` is a sub-rectangle only; render-to-image +
  upscale is a new camera/pipeline plus a coordinate remap). Rejected as too large.
- Writing canvas `width`/`height` from JS/web-sys is clobbered by wgpu on the next configure. Rejected.

**Chosen: a CSS-scaled wrapper in `play.html` only.** Wrap the canvas (`play.html` ~L223) in
`<div id="ironhold-scaler">` with `transform-origin: 0 0; transform: scale(k); width: calc(100% / k); height: calc(100% / k)`.
A small JS function computes `k = max(1, sqrt(hostCssW * hostCssH * dpr^2 / BUDGET))` and applies it; re-run from a
ResizeObserver on the host and a `matchMedia('(resolution: Ndppx)')` listener (DPR change). Bevy then sees (W/k, H/k) CSS px
× DPR physical, so physical pixels <= budget; logical and physical size shrink together, `scale_factor()` stays DPR,
`ui_scale_system` (computed from logical size) stays self-consistent, and **`ironhold_core` is untouched** (native is
unaffected by construction).

- `BUDGET` = **3.7 MP** (1440p = 3.69 MP: 1440p at DPR 1 is untouched; 4K at DPR 1.5 gets k ~ 1.5, ~2560x1440 rendered,
  ~54% fewer pixels than 8 MP). Lowering to ~2.1 MP (1080p) is a one-line change if profiling says so. Overridable with a
  `?px_budget=<megapixels>` query parameter (for testing and for a quick perf experiment). No `ProjectConfig` field: the
  host page is a deployment contract, not designer surface (`web_host_page_contract` memory); if one is ever wanted, copy the
  `ui_scale` pattern.
- k = 1 applies nothing. A zero-sized/hidden host leaves k = 1. Round k to 0.05 steps to limit text blur from fractional
  scaling. The result is a browser-upscaled (slightly soft) image above the budget, no change in logic resolution.
- Complementary, not part of this change: `Msaa::Sample2`/`Off` on web cameras would cut cost another 2-4x per pixel
  (separate decision; log if wanted).
- **Main risk (unverified):** pointer coordinate mapping under the CSS transform. winit derives pointer positions from the
  canvas element (`offsetX/Y` is in untransformed local space in Chrome, which should be correct; any
  `getBoundingClientRect`-based path would be scaled). **Must be verified in a real browser** before this phase is accepted:
  click a UI button and a world target at k > 1 and confirm they land where the cursor is. If it fails, fall back to
  logging the item as blocked and shipping A+B only.
- Testing: a headless Rust test cannot cover this (no DOM). Add a Playwright case to `test_web.py` (see
  `docs/dev/browser_tests.md`): viewport 3840x2160 (viewport-size emulation only; **do not use `device_scale_factor`** for
  DPR checks, see `responsive_ui.md` ~L375-381) asserting `canvas.width * canvas.height <= budget`, plus a click-position
  check. Real-DPR coverage is part of the manual playtest.
- Screenshot baselines at <= budget have k = 1, so they are unchanged; larger viewports differ.

## Tasks

- [ ] A: `GlobalAmbientLight` resource instead of entity; reset on scenes without `ambient:`; fix doc comments
- [ ] A: test — scene with `ambient:` sets the resource; a following scene without it resets to default; no `Camera`
      entity spawned by lighting
- [ ] A: audit the 37 ambient scenes for badly-tuned values (e.g. brightness that blows out now it applies); list
      affected projects for baseline re-capture
- [ ] B: `WarmupNoCull` marker + system change
- [ ] B: test — an entity with permanent `NoFrustumCulling` keeps it after warmup; a warmup-added one loses it; a mesh
      spawned during the window with permanent NFC keeps it
- [ ] C: `play.html` scaler wrapper + JS (`k`, ResizeObserver, matchMedia, `?px_budget=`); Playwright case; manual real-browser pointer check at k > 1
- [ ] Docs: `docs/20_data_formats.md` lighting table (behaviour now real; note default 150 lux), crate CLAUDE.md warmup
      sentences, `docs/dev` if a topic page covers warmup/canvas, backlog (move 3 entries to Done), `ocean_simulation_demo.md`
- [ ] CLI: `cargo check -p ironhold_cli` (no schema change expected; confirm)
- [ ] WASM dev build + playtest checklist; baselines for affected projects (own commit)

## Open questions

- A: accept the one-time look change in the 37 scenes (recommended), or add a per-scene opt-out? (Recommended: accept; there
  is no authored intent to preserve — nobody has ever seen those values take effect.)
- C: 3.7 MP or 2.1 MP default budget? (Recommended 3.7 MP; revisit after profiling a 4K/integrated-GPU run.)
- C: other embedders of the engine do not get the cap unless they copy the wrapper; document it next to the
  `#ironhold-canvas` host-page contract (backlog item).

## Acceptance criteria

- Given a scene with `lighting: (ambient: (0.0, 0.0, 0.0), ambient_brightness: 0.0)`, when loaded, then
  `GlobalAmbientLight` is black/0 and unlit surfaces are dark; and given the next scene has no `ambient:`, then the
  resource is back to Bevy's default.
- Given a particle pool created during the warmup window, when warmup ends, then the pool mesh still has
  `NoFrustumCulling`; given an ordinary mesh, then it loses it as before.
- Given a 3840×2160 canvas at DPR 1.5 on web, when the game runs, then the backing store is at most the budget (no more
  than ~3.7 MP) and the UI/input are still correctly laid out and hit-tested; on native nothing changes.
