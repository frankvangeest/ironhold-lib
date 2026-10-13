# Feature: Pre-release render fixes (ambient light, warmup NoFrustumCulling, canvas pixel cap)

_Status: Ready (plan-review done: architect, alignment, wasm-perf; all blockers folded in)_
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

- Replace the entity spawn with `commands.insert_resource(GlobalAmbientLight { color, brightness, ..default() })`
  (`color`, `brightness`, `affects_lightmapped_meshes`; `bevy::prelude`; Bevy re-copies it to the render world on change, so
  re-inserting is correct; headless test apps without `LightPlugin` are fine).
- **The insert must happen outside the `if let Some(lighting) = &scene.lighting` block** (plan-review blocker: 7 scenes have
  no `lighting:` block at all, and menu scenes are the usual next scene after a lit one). Compute
  `let ambient = scene.lighting.as_ref().and_then(..)` and always insert, resetting to `GlobalAmbientLight::default()`
  (Bevy's white / 80 cd/m^2, i.e. exactly today's look) when nothing is authored. `apply_lighting_v2` is called only from the
  Replace branch (overlays never reach it, and an overlay's `lighting:` stays ignored - say so in the docs).
- **`ambient_brightness` set without `ambient`:** apply the resource when *either* field is set (colour defaults to white),
  so the obvious authoring mistake is not silently dropped. Present colour without brightness uses the 150 default; absent
  both = Bevy default 80.
- **Same leak, same fix:** `shadow_map_size` / `point_shadow_map_size` (`scene_loader.rs:2991-2996`) are inserted only when
  `Some` and never reset, so a 4096 shadow map leaks into the next scene. Reset both to their defaults on Replace in the
  same place.
- No schema change: `SceneLightingV2.ambient` / `ambient_brightness` keep their names and the `150.0` default.
  Fix the `schema/scene_v2.rs` doc comment ("`AmbientLight::brightness`" → `GlobalAmbientLight`).
- **Visual side effect (needs a decision before merging):** 37 scenes set `lighting.ambient`, so their look will change
  (authored colour and brightness now apply; they were rendered with Bevy's default white/80 cd/m² while being tuned).
  Brightness default is 150 vs Bevy's 80. Affected project screenshot baselines must be re-captured
  (`--real-gpu`, only the affected projects, own commit). Alternative considered and rejected: silently scaling to keep
  the old look — it would make the documented values lie.
- Also removes the stray render-graph-less `Camera` entity the old code spawned (bevy_render warn on every such load);
  run the camera and split-screen tests afterwards, since a `Query<&Camera>` without a `Camera3d` filter used to count it.
- **Look change, audited by alignment-reviewer (all 37 scenes):** colours are linearised by `Color::srgba`, so most scenes
  get **darker** (dark tints x 80-360 vs today's white x 80); only `particles_demo/particles2` and
  `integration_tests/terrain_test` get brighter. **Re-tune before re-baselining:** `foliage_demo/main` (brightness 1.2,
  effectively zero ambient), `custom_materials/main` (15, same), and `blank_project/main` (the new-project template, 260);
  spot-check `primitive_world/start_menu` (60), `quick_scene` (80), `terrain_demo` (90), `3rd_person_game_demo/main` (90).
  A designer who wants exactly the old look writes `ambient: (1.0, 1.0, 1.0), ambient_brightness: 80.0` - put that line in the
  docs and in the RON comment of any re-tuned scene.

### B — warmup removes only what it added

`pipeline_warmup_system` (`lib.rs:480-503`) inserts `NoFrustumCulling` on every `Mesh3d` for 4 frames, then removes it
from **every** `With<NoFrustumCulling>` entity. `particle_renderer.rs:364/392` spawn pool meshes with a *permanent*
`NoFrustumCulling` (world-space vertices, identity transform), so a pool created inside the window is un-marked and then
culled against a stale AABB.

- Add a private marker component `WarmupNoCull`. While counting down, insert `(NoFrustumCulling, WarmupNoCull)` only on
  `Mesh3d` entities that **do not already have** `NoFrustumCulling`. At zero, remove `NoFrustumCulling` and
  `WarmupNoCull` only from `With<WarmupNoCull>` entities.
- Use `Without<NoFrustumCulling>` in the insert query (also stops re-inserting every frame), `try_insert`/`try_remove` for the
  deferred commands, and count the "warmup complete" log from `With<WarmupNoCull>`.
- **Rule to document** (crate CLAUDE.md + ocean B1 note): a permanent `NoFrustumCulling` goes in the *same spawn bundle* as
  `Mesh3d`; adding it later to a mesh warmup already marked would still be removed at zero unless the owner also removes
  `WarmupNoCull`. Add a one-line comment on `PipelineWarmup`: setting it to 0 directly skips the cleanup.
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
- Round k **up** to the next 0.05 (ceil; its value is fewer reconfigures during a drag-resize, not less blur), so the "<= budget" criterion holds; use a CSS custom property
  (`calc(100% / var(--k))`); compute k from `#game-host`, never from the scaler (no ResizeObserver feedback loop); comment in
  `play.html` that the transform creates a containing block for any future `position: fixed` child and may affect a future
  `canvas.requestFullscreen()`.
- Winit pointer positions come from `MouseEvent.offsetX/Y` (`winit-0.30.12 web_sys/event.rs:83-97`, element-local, transform
  aware); `getBoundingClientRect` is only used by `canvas.position()`. The pointer risk is therefore likely fine but is still
  verified manually (gate below).
- **`test_web.py::test_real_hidpi` (~L816-858) will break** (DPR 2/3 `no_viewport`: asserts engine `window_size` ==
  canvas `getBoundingClientRect()` and `canvas.width` == CSS width x factor; at k > 1 both fail). Run it with `?px_budget=`
  high enough to disable the cap, or rewrite it to use `clientWidth`/`offsetWidth` and the applied k.
  `assert_canvas_fills_viewport` keeps passing.
- Docs: add the scaler to the `#ironhold-canvas` host-page contract and `docs/dev/browser_tests.md`; one sentence in
  `docs/20` "Responsive UI: `ui_scale`" (~L4751): above the budget Bevy's logical window shrinks by k (4K at DPR 1.5 goes
  from 2560x1440 to ~1707x960 logical), so projects *without* `ui_scale` show fixed-`Val::Px` UI k times larger.
- **Logical floor (wasm-perf blocker):** the cap shrinks Bevy's *logical* window by k, so projects without `ui_scale` show
  `Val::Px` HUD/labels k times larger, and a Retina laptop (1512x982 @2) would fall to ~1192x774 logical at 3.7 MP, below the
  1280x720 the HUDs are authored for. Use `k = min(k_budget, max(1, min(cssW / 1280, cssH / 720)))`: small HiDPI laptops may
  go slightly over budget (~4.3 MP in that example), which is fine. Document the visual-size change.
- **Budget in exact pixels:** `BUDGET_PX = 3_686_400` (2560x1440), so "1440p at DPR 1 is untouched" cannot depend on
  whether "3.7 MP" means 3.7e6 or 3.7*2^20. `?px_budget=` takes megapixels (or `off`), converted to pixels. 3.7 MP stays the
  default: 2.1 MP would push every Retina laptop below the logical floor.
- **Where it runs:** compute and apply k in the existing classic `<script>` IIFE (`params`/`ironholdStartupFailed`, ~L271),
  **before** the module `init()`, otherwise the first surface configure allocates full-size MSAA targets (~350 MB at 8 MP)
  and immediately reallocates them. Re-arm the `matchMedia('(resolution: Ndppx)')` listener after every change (browser zoom
  changes DPR). The host ResizeObserver resizing a deeper wrapper causes no RO loop error. The Playwright pixel check needs
  a small tolerance for device-pixel snapping.
- Firefox only (coalesced pointer events take deltas from `offsetX` differences): un-locked drag-orbit is up to k times
  less sensitive at k > 1 (`winit event.rs:131-137`). Minor; add to the playtest checklist.
- **MSAA is out of scope:** WebGPU only allows sample counts 1 and 4, so `Msaa::Sample2` is not a web option (the earlier
  "Sample2 or Off" wording was wrong); `Off` needs FXAA/SMAA as its own visual decision. Log as a separate backlog item.
- k = 1 applies nothing. A zero-sized/hidden host leaves k = 1. Round k to 0.05 steps to limit text blur from fractional
  scaling. The result is a browser-upscaled (slightly soft) image above the budget, no change in logic resolution.
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

- [ ] A: `GlobalAmbientLight` resource instead of entity, inserted outside the `lighting` `if let` (reset to default when
      unauthored, either-field rule); reset the two shadow-map resources too; fix `scene_v2.rs` doc comments (cd/m^2, not lux;
      add the missing doc on `ambient`)
- [ ] A: tests - scene with `ambient:` sets the resource; explicit black; brightness-only; a following scene with
      `lighting: None` AND one with a `lighting` block but no ambient reset to default; shadow-map reset; no `Camera` entity
      spawned by lighting; run the camera/split-screen suites
- [ ] A: **merge gate** - re-tune `foliage_demo`, `custom_materials`, `blank_project` (+ spot-check the others listed
      above), fix the `docs/20` lighting rows (L286-287: cd/m^2, "omitted = Bevy default white/80; 150 applies only when
      `ambient` is set", drop the "clip at 1.0 / 50-300" guidance) and the near-black example (~L319-320); list affected
      projects for baseline re-capture
- [ ] B: `WarmupNoCull` marker + system change (`Without<NoFrustumCulling>` insert filter; one `remove::<(NoFrustumCulling, WarmupNoCull)>()`)
- [ ] B: tests - an entity with permanent `NoFrustumCulling` keeps it after warmup; a warmup-added one loses it; a mesh
      spawned during the window with permanent NFC keeps it
- [ ] C: `play.html` scaler wrapper + JS in the classic script before `init()` (`k` with logical floor, exact-pixel budget, ResizeObserver, matchMedia re-arm, `?px_budget=`); Playwright case; manual real-browser pointer check at k > 1
- [ ] Docs: `docs/20_data_formats.md` lighting table (behaviour now real; note default 150 lux), crate CLAUDE.md warmup
      sentences, `docs/dev` if a topic page covers warmup/canvas, backlog (move 3 entries to Done), `ocean_simulation_demo.md`
- [ ] C: fix/run `test_real_hidpi` with the cap disabled or rewritten (see above)
- [ ] CLI: `cargo check -p ironhold_cli` (no schema change expected; confirm)
- [ ] WASM dev build + playtest checklist; baselines for affected projects (own commit)

## Open questions

- A: accept the one-time look change in the 37 scenes (recommended - it is the only way to make the documented contract
  true) with the mandatory re-tune above. Caveat from alignment-reviewer: some values (e.g. `custom_materials`: "too-high
  ambient washes out the vein contrast") may have been tuned when ambient still worked, so intent may exist; verify via git
  history while auditing.
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

## Plan-review log (2026-10-12)

- **system-architect: GO** after two fixes (insert outside the `lighting` `if let`; `test_real_hidpi` will break at k > 1) - both folded in.
- **alignment-reviewer: GO** with conditions (re-tune `foliage_demo`/`custom_materials`/`blank_project` before re-baselining;
  docs/doc-comment wording; either-field rule) - folded in; all 37 scenes audited.
- **wasm-perf-reviewer: GO** for A and B; C after the logical floor and the `test_real_hidpi` task - folded in. Pointer
  mapping verified from winit 0.30.12 sources (`offsetX/Y`); still gated on a manual real-browser click check.
- **Separate backlog items to log:** MSAA policy on web (Off + FXAA/SMAA, WebGPU supports only 1 or 4 samples); document the
  scaler in the host-page contract.
