# Feature: Responsive UI (canvas fit, reference-resolution scale, orientation event)

_Status: Ready (plan-review rounds 1-3 + final check passed 2026-10-11; Phase 3 is gated on `ui_set_visible.md` shipping, which needs its own plan-review)_
_Planned at: `e3a2ff2` (2026-10-10)_

Sources: UX + system-architect consult on the main-scene baselines (2026-10-10, see the `### UI`
section of `planning/backlog.md`), the Bevy 0.18.0 source in the cargo registry, and plan-review
round 1 (system-architect + ux-gamedesigner-reviewer, 2026-10-11). Facts marked **[verified]** were
read in source; **[spike]** items must be confirmed before code commits to them.

## Decisions (Frank, 2026-10-11, answering review round 1)

1. **Portrait/landscape layout switching uses a new `SetUiVisible { id, visible }` action** (written
   `SetUiVisible(id: "...", visible: ...)` in RON), not overlays
   (overlays are one global slot, each adds a click-blocking backdrop, and would unload a pause
   menu on rotate) and not multi-slot overlays.
2. **Determinism: the orientation event goes through the presentation channel (`UiEvent`)**, plus a
   `--strict` validate warning when a `window.orientation:*` binding contains non-UI actions. It is
   *not* recorded as a replay input.
3. **Canvas readiness signal is a `data-ready` attribute**, not "a `<canvas>` exists".
4. **Schema shape is the reviewers' suggested `ui_scale:` block** (below): no `Some(`, no `None`
   variant, `FitMin` renamed `Fit`, optional engine-chosen `portrait_reference`.
5. **`reference` is required, `fit` defaults to `Fit`** (both reviewers' "explicit/required"
   concern is met by `reference` being required; a block the designer wrote is an opt-in).

Decisions 6-8 (initial hidden state, initial orientation, `data-ready` source) are under "Decisions
from the remaining questions" below. Round-3 decisions (Frank, 2026-10-11):

9. **P1 = B: ActionBar and panels stay out of the swapped layouts.** `SetUiVisible` cannot target
   them (validate rejects it), so in the portrait recipe the bar and panels **keep one position in both
   layouts and must fit inside both reference rects** (for the default references: `x + width <= 720`
   and `y + height <= 720`, i.e. inside the smaller of the two). Phase 3 ships with that documented
   limit plus an acceptance check on the `3rd_person_game_demo` retrofit (bar on screen in portrait).
   The existing "ActionBar/panels flow inside a Group" backlog item is the follow-up that lifts the
   limit; it is **not** a prerequisite.
10. **P2: `SetUiVisible` is split** into `planning/features/ui_set_visible.md`, which Phase 3 depends on.

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| 1 | Canvas fit + `data-ready` signal + responsive test harness | Queued | — |
| 2 | `ui_scale` project setting (reference-resolution `UiScale`) + pixel-site audit | Queued | — |
| 3a | `SetUiVisible` action (own plan: `ui_set_visible.md`) | Queued | — |
| 3 | Orientation event + `portrait_reference` (depends on 3a) | Queued | — |
| 4 | Follow-ups (world-label scale, safe area, touch, mobile render cap, Group v2 re-scope) | Queued | — |

## What

A game designer can author one UI and have it stay usable on a 1920x1080 monitor, a 1024x768
tablet, an 844x390 phone in landscape and a 390x844 phone in portrait. Three small, opt-in pieces:
the web canvas fills the browser viewport; an optional project setting scales every UI pixel value
against a reference resolution; and a presentation-only orientation event plus a `SetUiVisible`
action let a project show a portrait or landscape layout. Projects that set nothing behave exactly
as today.

Designer mental model (goes in `docs/20`): **`ui_scale` = how big things are; flex `Group` = where
things go; orientation event + `SetUiVisible` = which layout is showing.**

## Why

The flex `Group` (v1, shipped) only helps if the UI root actually changes size. Today it never does:
`start_app` uses the default window (`lib.rs:848`), `play.html` hosts a fixed 1280x720 canvas, and
there is no `UiScale` or resize handling anywhere in `crates/`. Even once the canvas resizes,
`ActionBar`, the four panels and every composite widget hardcode pixel sizes and font sizes
(`scene_loader.rs:2066, 2198, 2258, 2407, 2511, 2540`), so per-leaf `Percent` (Group v2) cannot fix
them; a global scale can. This unblocks `mobile_ui_demo` v1/v2, Group v2 and the HUD retrofits.

## Bevy 0.18 facts this design relies on

- **`UiScale(pub f32)`** is a resource (`bevy_ui-0.18.0/src/lib.rs:119`, default 1.0), affecting
  only fixed `Val::Px` values. **[verified]** Text scale already includes it (`widget/text.rs:355`),
  so hardcoded font sizes scale too. `Percent`/`Auto` and Group flex maths are untouched.
- **Root `Percent(100)` under a scale [verified, architect]:** `UiScale` is multiplied into each UI
  root's scale factor (`update.rs`, `propagate_ui_target_cameras`), so layout runs in
  `physical / (dpr * UiScale)` units and a root `Percent(100)` spans `window / UiScale`. The 3-node
  test at scale 0.5 / 1.0 / 2.0 stays as a **regression test**, not a spike.
- **`Window.fit_canvas_to_parent`** (`bevy_window-0.18.0/src/window.rs:259`, default `false`) sets the
  canvas CSS `width/height: 100%` on wasm (`bevy_winit-0.18.0/src/system.rs:104-111`). **[verified]**
  The parent must be explicitly sized, and the 44px top bar must be excluded from the height or the
  inline `height:100%` overflows `body`. `Window.canvas: Option<String>` (`window.rs:251`) selects an
  element by CSS selector.
- **`WindowResized`** is `#[derive(Message)]` **[verified]**; read it with `MessageReader`
  (`event.rs:37`, `window` entity + logical `width`/`height`).
- **`prevent_default_event_handling`** defaults to `true` (`window.rs:489`): touch scroll/zoom on the
  canvas is swallowed. Relevant to Phase 4 touch.
- **`WindowResolution::scale_factor_override`** (`window.rs:921`) changes the *render* scale for
  everything; rejected in favour of `UiScale`, which touches UI only.
- DPR: winit keeps logical size = CSS size and multiplies physical size by devicePixelRatio. Nothing
  here needs DPR handling beyond testing it.
- On native, `canvas`/`fit_canvas_to_parent` are documented no-ops and native resizing already sends
  `WindowResized`, so the `WindowPlugin` override is set **unconditionally** (one less cfg branch)
  and native behaves the same.

## Approach

### Phase 1 — canvas fit (S) + responsive test harness (S)

1. `start_app` (`lib.rs:848`): `WindowPlugin` override with `fit_canvas_to_parent: true`. Whether to
   also set `canvas: Some("#ironhold-canvas")` is tied to the readiness signal below.
2. **Readiness signal (decision 3).** Today `play.html` hides its loading overlay on
   `querySelector('canvas')` (~line 267) and `test_web.py:215` does `wait_for_selector("canvas")`;
   both treat "a canvas exists" as "the engine started". If the canvas element is authored in the
   HTML, both fire before the wasm loads. So: the web runner sets a `data-ready` attribute (on
   `body`, set by `ironhold_web` once the first frame has rendered, or by `play.html` when the
   `#debug-state` first has content) and **both** `play.html` and `test_web.py` switch to waiting
   on `[data-ready]`. Keep `canvas: None` unless the spike shows a specific element is needed.
3. `play.html`: `html, body { height: 100% }`; a flex column with the 44px bar and a flex-grow host
   (`flex: 1; min-height: 0`) so the bar is excluded from the canvas height. `?bar=hidden` and
   `?testing=1` still remove the bar, making the host the full viewport. Folds in the queued
   "Fit the game canvas" item.
4. Spike results to record first: what the fixed canvas does on a real phone and tablet; **where the
   1px dark edge on every baseline comes from** (canvas/body border?) and keep it if keeping is free.
5. Test harness: see the Playwright section.

### Phase 2 — `ui_scale` project setting (M)

New optional `ProjectConfig` field (additive, `#[serde(default)]`, no schema version bump; the
struct is `deny_unknown_fields`, an additive optional field is safe). Named `ui_scale`, not `ui`,
so designers do not paste it into a scene's `ui:` element list. Written without `Some(...)`
(`ron_lint`'s `implicit_some`):

```ron
ui_scale: (
    reference: (1280.0, 720.0),            // required: the resolution the UI is authored at
    fit: Fit,                              // Fit (default) | FitWidth | FitHeight
    min_scale: 0.5,                        // default 0.5
    max_scale: 2.0,                        // default 2.0
    snap: 0.25,                            // default 0.25; 0.0 disables snapping
),
```

Leaving the block out means off (`UiScale` stays 1.0); there is no `None` variant.

- **Degenerate window sizes (Frank, 2026-10-11: a minimized PC window can report 0x0).** Both
  `ui_scale_system` and the orientation logic must treat any `width <= 0 || height <= 0` (or
  non-finite) as "no information": **ignore the event, keep the last applied scale and last
  orientation, write nothing.** Without this a minimize would compute `0 / reference = 0`, clamp to
  `min_scale`, and on restore re-layout the whole UI (and a 0 aspect ratio would flip the
  orientation to portrait, then back). One shared helper `valid_window_size(w, h) -> Option<Vec2>`
  is used by both systems and by the `window_size` debug-state field (Bevy owns
  `fit_canvas_to_parent`, so Phase 1 is unaffected). A restore must return to exactly the pre-minimize scale and
  fire no orientation event. On wasm the equivalent case is a hidden host or background tab
  (canvas 0px); same rule.
- One `Update` system `ui_scale_system`, reading `MessageReader<WindowResized>` plus a project-load
  trigger. `s = fit(window / reference)`, **snapped downward** to `snap` (never nearest: 0.88 ->
  1.0 would overflow), then clamped to `[min_scale, max_scale]`, and written to `UiScale` **only
  when the value changed** (the change-detection rule). If the block is absent but `UiScale != 1.0`
  (project hot-reload), write 1.0 back. Presentation only: never read by simulation, no determinism
  impact, never in `FixedUpdate`.
- **Visible clamp.** When the clamp changes the value, log one warning per window-size class:
  "window 390x844 needs scale 0.30 but `min_scale` is 0.50; UI will overflow".
- **Hand-converted pixel sites (architect blocker).** Systems that compute `Val::Px` from window
  pixels divide by `window.scale_factor()` only, so they break under any `UiScale != 1.0`:
  the split-viewport player label (`capabilities/camera.rs` ~874) and the target HUD readout
  (~987). Fix by dividing by the UI target's scale (`ComputedUiRenderTargetInfo`), and add an
  audit task: grep every `/ ...scale_factor` that feeds a `Val::Px`. World labels, nameplates and
  popups are `Text2d` and unaffected.
- **Validation in `ProjectConfig::validate()` under `schema/`** (shared by the CLI and the runtime;
  no CLI-only checks): `reference` components > 0 and finite; `min_scale > 0`; `min_scale <= max_scale`;
  `snap >= 0`; `portrait_reference` (Phase 3) > 0 and finite. Each message names the field and
  gives the fix ("`ui_scale.min_scale` (1.5) > `max_scale` (1.0): swap them"). Rows in
  `docs/15_authoring_tools.md`.
- **Split-screen:** one global scale, derived from the whole window. All RON UI renders through one
  full-window `Camera2d`, so this is consistent once the pixel sites above are fixed. Documented in
  the split-screen section of `docs/20`; no per-camera scale.

### Phase 3 — orientation event + `portrait_reference` (M; depends on `ui_set_visible.md`)

**`SetUiVisible` is its own plan: `planning/features/ui_set_visible.md`** (split out 2026-10-11, Frank).
It carries the action, the `UiNodeId` component, the `Display::None` mechanism, the validate checks
(any-scene id lookup, panel/`ActionBar`-subtree rejection), `Action::is_presentation_only()` and the
conditional `initially_hidden` field. **Phase 3 here cannot start until that plan has shipped.** The
recipe below uses the action as specified there.

**Orientation event (decision 2).** `window.orientation:portrait` / `window.orientation:landscape`,
sent as a **`UiEvent`** (the presentation channel), derived from `width/height` against 1.0 with
hysteresis (portrait at < 0.95, landscape at > 1.05).
- **New `UiEvent` variant required (architect B1).** `UiEvent` has one variant today,
  `ButtonPressed(String)` (`runtime/messages.rs:4-6`), and both interpreters prefix it
  (`fsm_interpreter.rs:36-37`, `entity_fsm_interpreter.rs:25-26`: `ui.button_pressed:{}`), so a naive
  send would arrive as `ui.button_pressed:window.orientation:portrait`. Add
  `UiEvent::Presentation(String)` whose payload is used **as the event name unchanged**. Sites that
  destructure `UiEvent` assuming one variant and stop compiling once it has two (architect, round 3):
  `fsm_interpreter.rs:36`, `entity_fsm_interpreter.rs:25`, `dialogue.rs:110` (the
  `|UiEvent::ButtonPressed(t)|` closure; line 90 is only the parameter) and
  `tests/same_frame_order_tests.rs:75`; other test uses are `matches!` and fine. Decision: the event is
  delivered to the global FSM and to entity behaviours alike (they already share the `UiEvent` reader);
  document it. The `{self}`/`{target}` pass-through for `SetUiVisible` is owned by `ui_set_visible.md`.
- **Ordering (architect N1).** The interpreter reads UI, then Game, then Scene events each frame, so an
  orientation event sent in the `scene.ready` frame runs its actions *before* any `scene.ready` rule.
  Order the orientation system `.after(spawn_scene_v2).before(fsm_interpreter_system)` and document
  "never set layout visibility from a `scene.ready` rule" (it would undo the swap). The flash spike
  covers this one-frame gap too.
- **Overlay loads emit no `Ready`** (`scene_loader.rs:1332`), so Groups inside an overlay never
  receive the re-send; document it.
- **Re-sent on every `SceneEvent::Ready`** (not just at load; layout state resets on `LoadScene`, and
  a load-time send may arrive before the FSM is active), and the engine writes a
  `window_orientation` GameVariable, listed in `docs/20` next to the other engine-written variables.
  Bindings go in `global_on`.
- **One shared orientation resource.** Orientation and `portrait_reference` selection need the *same*
  hysteresis result, so a single `WindowOrientation` resource (written change-guarded) is the source
  for both; `ui_scale_system` reads it, and runs `.after` the system that updates it, so a flip and
  its reference swap land in the same frame. First read when the aspect is already inside the
  0.95-1.05 band: `width < height` is `portrait`, otherwise `landscape` (no prior state to be sticky to).
- **Source of truth (decision 7, clarified):** orientation is computed from the `Window` resource's size
  at every `scene.ready` (so a project never starts from a guess) and thereafter updated on
  `WindowResized` threshold crossings. Only a degenerate
  size (see the 0x0 rule in Phase 2: `valid_window_size` returns `None`) falls back to the **last
  known orientation, or `landscape` if none yet** (every existing project is authored landscape).
  A minimized (0x0) window therefore never flips orientation, never emits an event, and a restore
  fires nothing unless the real size crosses the hysteresis band.
- `ironhold validate` rejects any `window.orientation:` payload other than exactly `portrait` or
  `landscape` (a typo like `Portrait` would silently never fire), and `--strict` warns when such a
  binding contains an action that is not presentation-only. "Presentation-only" is defined **once** as a
  shared predicate under `schema/` (introduced by `ui_set_visible.md`; `Action::is_presentation_only()`, an **exhaustive `match` with no
  `_` arm** so every new action forces a decision), used by the check and listed in
  `docs/15` (decide per action, e.g. `SetVariable` is *not* presentation-only by default because
  variables can drive gameplay).
- **`window_orientation` GameVariable (architect N5):** kept (needed so a rule or label can read the
  current orientation), but dialogue conditions read `GameVariables`, so a per-device value can branch
  gameplay. Document it as presentation-only; a `--strict` warning is out of scope.
- `gameplay_fixed_tick_pipeline.md` cross-link: the event is window-derived and differs per device, so
  gameplay reactions are a documented anti-pattern, not a recorded input.

**Designer recipe (goes in `docs/30`, shown as the full block, not prose):** author both HUDs in the
same scene as id'd `Group`s, with **one shared `ActionBar` (and any panels) outside them**. Documented
limit (decision 9): the bar and panels have only an absolute `position:` and no anchor, so they keep one
position in both layouts and must fit inside both reference rects (default references: `x + width <= 720`,
`y + height <= 720`). On today's `3rd_person_game_demo` main (bar at 416,580; dialogue panel 1220 wide)
this means moving the bar and narrowing the panel as part of the retrofit:

```ron
// Layout only. Never set layout visibility from a `scene.ready` rule: it runs after
// this and would undo the swap.
global_on: [
    ( event: "window.orientation:portrait", do_actions: [
        SetUiVisible(id: "hud_landscape", visible: false),
        SetUiVisible(id: "hud_portrait",  visible: true),
    ]),
    ( event: "window.orientation:landscape", do_actions: [
        SetUiVisible(id: "hud_portrait",  visible: false),
        SetUiVisible(id: "hud_landscape", visible: true),
    ]),
],
```
The field names are `event` / `do_actions` (`FsmEventBinding`, `schema/project.rs:137-140`,
`deny_unknown_fields`; a wrong name fails the whole `state_machine.ron` parse, confirmed against
`ui_demo/logic/state_machine.ron:7`). `global_on` has no `condition`, so one rule pair per
orientation is the pattern. `window_orientation` (the variable) is an optional footnote in the docs,
not part of the core mental model.

The validate rejection of panel/`ActionBar` targets and its message live in `ui_set_visible.md`. In this
plan a `--strict` warning flags an id that exists **only inside an overlay** but is targeted by a
`window.orientation:` binding, since overlays get no re-send and it would never switch.

**Flash rule (decision 6):** once the orientation swap works (it needs `SetUiVisible`, so this check
happens during Phase 3, not Phase 0), screenshot the first frames of a scene with both HUDs authored in a
portrait window, including the `scene.ready` one-frame gap. If a flash is visible, **Phase 3 adds** an
authored `initially_hidden: bool` on `Group` and leaf nodes, using `ui_set_visible.md`'s `Display::None`
mechanism (default `false`, unlike the existing panel field's `true`, which hides with
`Visibility::Hidden`; state both defaults in `docs/20`); a designer sets it on the non-default layout. Do **not** add an engine-side
`orientation:` field on `Group` (the deferred layout-variant primitive). If no flash is visible, add
nothing.

**`portrait_reference: Option<(f32, f32)>`** added to `ui_scale` here (additive on a
`deny_unknown_fields` struct, no schema bump). Chosen engine-side by aspect with the same
hysteresis. This is the fix for phones: a 390x844 portrait window against a 1280x720 reference
needs 0.30, below the 0.5 floor; against a 720x1280 portrait reference it needs ~0.54. The 0.5
floor stays; scaling cannot reflow a 1280-wide HUD into 390px, the portrait layout does.

### Phase 4 — follow-ups (each its own backlog entry when picked up)

- Apply the scale to world-label `depth_scale_factor`'s consumer (`lib.rs:722`) behind the
  change-detection guard. Until then document that nameplates/world bars do not scale (at 1920x1080
  the HUD grows 1.5x but labels do not).
- Safe-area insets in `play.html` only: `viewport-fit=cover` plus `env(safe-area-inset-*)` padding.
- Touch: Bevy's UI focus folds touch into `Interaction` but nothing in core reads `Touches`, and
  orbit / click-to-target / flycam are mouse-only (`camera.rs:218`, `targeting.rs:202`,
  `flycam.rs:77`). Settled by the Playwright touch test before any claim is made.
- **Mobile render cost (wasm-perf):** a full-viewport canvas at DPR 3 renders at native phone
  resolution; cap the render scale for mobile.
- Camera FOV policy for portrait.
- Re-scope "Flex `Group` v2": leaf `Percent`/`max_width` becomes polish after Phases 1-2, and
  `SetUiVisible` (`ui_set_visible.md`) satisfies its runtime show/hide item. The "ActionBar/panels flow inside a
  Group" backlog item remains the prerequisite for bottom-centred bars.

## Playwright screenshot impact

`test_web.py` runs one context at a fixed `viewport={"width": 1280, "height": 720}` (`:552`), opens
`play.html?...&testing=1` (`:209`), and diffs full-page screenshots against `screenshot_baselines/`
with a hard size check (mismatch returns `1.0`, `:230`).

1. **Existing baselines must not move.** `?testing=1` hides the top bar, so the host is the full
   1280x720 viewport and the canvas is pixel-identical. Gate: run the full suite after Phase 1 and
   after Phase 2 *without* `--update-baselines`; any baseline diff is a bug. Projects without a
   `ui_scale` block keep `UiScale` at 1.0. **`ui_demo`'s `ui_scale` reference stays 1280x720** so its
   main baseline remains a real regression check.
2. **Readiness.** `test_web.py:215` and `play.html` move to `[data-ready]` (decision 3). This is a
   change to the harness's own wait condition, so run the suite once on the `data-ready` change
   *alone* before the canvas-fit change, to separate the two failure causes.
3. **Host sizing is the likely break.** An unsized parent makes the canvas 0px or wrong-size; the
   size check flags it loudly. The 1px edge: its pixel share (~0.4%) is under the 4% threshold, so a
   change would pass silently. Decide deliberately (Phase 1 spike) and record it.
4. **`--responsive` mode, kept separate** (do not multiply the main loop):
   - New context per entry via `new_context(viewport=..., device_scale_factor=..., is_mobile=..., has_touch=...)`.
   - Matrix: 1920x1080, 1024x768, 768x1024, 844x390, 390x844 (`is_mobile`, `has_touch`). **No DSF
     emulation entry** (it makes winit see a CSS/dpr window, see Phase 1d); real HiDPI is tested with
     `--force-device-scale-factor` launches instead (x2, x3).
   - Projects: `ui_demo` (with a `ui_scale` block and a portrait variant) plus a retrofitted HUD
     scene (`3rd_person_game_demo` main).
   - Baselines in `screenshot_baselines/responsive/{project}_{scene}_{W}x{H}.png`, not under
     `scenes/` (that folder feeds the gallery). GPU-captured only (`--real-gpu`, port 8001).
5. **Settle after resize:** `page.set_viewport_size`, then frame-count from the post-resize frame
   (`SCREENSHOT_SETTLE_FRAMES = 120`, `:81`), never wall-clock sleeps.
6. **Two new `#debug-state` fields:** `window_size` (logical) and `ui_scale`. Tests assert the scale
   numerically and wait for a resize to land. The 4% pixel threshold cannot catch small overlaps or
   clipping, so also assert from debug state that no UI root/top-level node rect (found by `Name`)
   extends outside the window; per-id rects come after `ui_set_visible.md` ships (`UiNodeId`), as an
   optional stretch under `?testing=1`. Also a window-orientation field so
   the portrait recipe is asserted, not eyeballed.
7. **Touch test:** in the `has_touch` context `page.touchscreen.tap` a button (`quick_scene` `Dance`,
   as `test_button_fires_action`). Settles whether taps reach Bevy UI; a failure is a recorded input
   finding, not a blocker for Phases 1-3.
8. **Docs:** `docs/dev/browser_tests.md` (matrix, `--responsive`, baseline folder, the readiness
   attribute, the "never re-baseline to fix a Phase 1/2 diff" rule). The blank-project template and
   `/new-project` do **not** gain a `ui_scale` block.

## Tasks

- [ ] Phase 0 spike: 1px edge origin; fixed-canvas behaviour on a real phone/tablet; whether `update_debug_state` is
      gated (decision 8); what a minimized wasm host reports. (Native Windows is already verified: winit
      0.30.12 sends `Resized` 0x0 on minimize, bevy_winit writes `WindowResized{0,0}`, Bevy guards camera
      extraction/viewport/projection at 0, and our code never divides by window size: `lib.rs:656` only halves it.)
- [x] Phase 1a: `data-ready` signal; `play.html` + `test_web.py` switch to it; suite green alone. Done
      2026-10-11 (`7449a77`): set from `#debug-state`'s first content in `play.html` (decision 8's second branch:
      `update_debug_state`/`sync_debug_state_to_dom` run ungated), no Rust change. Full suite `--real-gpu
      --skip-build` against the existing release `pkg/`: 56/59, the 3 failures are **pre-existing baseline
      drift, identical with the old harness** (`foliage_demo_main` 5.5%, `local_coop_demo_room10` 8.4%,
      `pause_nav/02_main_scene` 4.4% before vs 4.5% after). The run also auto-wrote an unreferenced
      `local_coop_demo_room11` baseline (deleted, not part of this change).
- [x] Phase 1b: `WindowPlugin` override (unconditional), `play.html` flex host; suite green, no baseline change.
      Done 2026-10-11: `start_app` sets `Window.canvas = "#ironhold-canvas"` + `fit_canvas_to_parent: true`;
      `play.html` is a `100dvh` flex column (bar `flex: none`) with a `.game-host` (`flex: 1 1 0; min-height: 0`)
      holding the authored `<canvas id="ironhold-canvas">`. Deviation from the plan's "keep `canvas: None`":
      the authored canvas + selector keeps the 44px bar out of the canvas height (Bevy's inline
      `height: 100%` would otherwise overflow `body`); Phase 1a's `data-ready` is what makes an
      authored canvas safe. WASM dev build 2m21s. Full suite `--real-gpu --skip-build` on the dev build:
      56/59 with the **same 3 pre-existing failures and identical numbers** (5.5% / 8.4% / 4.4%); across
      the 40 scene baselines the only delta over 0.3 points is `3rd_person_game_demo_character_select`
      0.37% -> 0.01% (dev vs release build noise). Screenshot sizes unchanged (the size check would have
      returned 1.0). Requires a `pkg/` built from the same commit as `play.html` (documented in
      `docs/dev/browser_tests.md`).
      **Resize check (one-off Playwright script, dev build, ui_demo):** the canvas tracks the viewport
      exactly at 1280x720, 800x600, 1920x1080, 390x844 and 844x390, back to 1280x720; with the top bar it is
      viewport minus 44px high and starts at y=44; one canvas, no page scroll.
- [x] Phase 1c: `--responsive` harness, debug-state fields, first matrix, touch test. Done 2026-10-11:
      `DebugState.window_size` / `ui_scale` (+ `json_f32` guard, 2 core tests in `action_tests.rs`, 24/24 in
      that file); `test_web.py --responsive` (resize-in-place test, touch test, 5-viewport matrix x
      `ui_demo` + `3rd_person_game_demo` main, baselines in `screenshot_baselines/responsive/`, and two
      real-HiDPI tests). Run against its own baselines: 14/14 (`ui_demo` 0.00%, the 3D scene 0.29-2.85%;
      its 768x1024 entry is the closest to the 4% threshold, watch it). Not done (deferred, optional in the
      plan): per-node UI rect dumps / "no root rect outside the window" assertion.
      **Finding: touch taps reach Bevy UI buttons** (`touchscreen.tap` on quick_scene's Dance button fires
      `PlayAnimation("dance")`); the mobile items can assume UI taps work. World-side orbit/click-to-target
      are still mouse-only.
- [x] Phase 1d (was proposed as a HiDPI fix): **resolved as not a bug, 2026-10-11.** Phase 1c's first matrix
      had a DPR-2 entry that showed the engine at half the CSS size (`window_size` `[640,360]` for a 1280x720
      viewport, UI 2x too large and cut off) and was briefly logged as a bug. Root cause: **Playwright's
      `device_scale_factor` emulation makes ResizeObserver's `devicePixelContentBoxSize` report the CSS
      size** (an 800x600 element reports `[800,600]` at DSF 1, 2 and 3 on a page with no engine at all), and
      winit 0.30's web backend reads exactly that for the physical canvas size. With a **real** device
      scale factor (`--force-device-scale-factor` + `no_viewport`) the engine is correct: x2 gives CSS
      927x403, backing 1854x806, `window_size` 927x403; x3 gives CSS 608x231, backing 1824x693,
      `window_size` 608x231. The DPR-2 entry was replaced by `test_real_hidpi` (x2 and x3, passing) and the
      backlog bug was removed. **Do not use `device_scale_factor` for DPR checks.** Still open for Phase 4:
      the backing store is CSS x dpr, so a DPR-3 phone renders 9x the pixels (the x3 run: 1824x693 for a
      608x231 window); that is the mobile render-cost cap.
- [ ] Phase 2: `ui_scale` schema + `ProjectConfig::validate()` + `ui_scale_system` (snap down, clamp,
      write-on-change, reset-to-1.0, clamp warning) + `ui_demo` block (1280x720 reference).
- [ ] Phase 2: pixel-site audit (grep `/ scale_factor` feeding `Val::Px`); fix `camera.rs` ~874 and ~987; test both.
- [ ] Phase 2 gate: full suite green with no baseline changes; responsive baselines captured.
- [ ] **Prerequisite for Phase 3:** `ui_set_visible.md` planned, reviewed and shipped (`SetUiVisible`,
      `UiNodeId`, validate checks, `Action::is_presentation_only()`).
- [ ] Phase 3: `UiEvent::Presentation(String)` + the four named destructure sites (`fsm_interpreter.rs:36`,
      `entity_fsm_interpreter.rs:25`, `dialogue.rs:110`, `tests/same_frame_order_tests.rs:75`);
      `WindowOrientation` resource + orientation system ordered `.after(spawn_scene_v2).before(fsm_interpreter_system)`
      and before `ui_scale_system`; `window_orientation` variable + re-send on `SceneEvent::Ready`;
      first-frames flash check with both HUDs in a portrait window, adding `initially_hidden` only if
      a flash shows (decision 6); `portrait_reference`; validate `window.orientation:` payload + `--strict` non-presentation-action and
      overlay-only-id warnings; portrait variant in `ui_demo`; `3rd_person_game_demo` retrofit with the bar
      and panels moved inside both reference rects.
- [ ] Tests (core): scale formula (all fits, clamps, snap-down, no-write-when-unchanged, reset to 1.0),
      `valid_window_size` (0x0, 0xN, Nx0, NaN, inf all ignored; scale and orientation unchanged and
      no event/write; restore returns to the exact prior scale), orientation hysteresis, first read
      inside the band, re-send after scene change; validate codes; `cargo check -p ironhold_cli`.
- [ ] Docs: `docs/20` (ProjectConfig row; "Responsive UI" section with the mental-model table and
      known limits: world labels, split-screen, the clamp, absolutely positioned elements not
      hugging edges at other aspect ratios; engine-written `window_orientation` variable; "preview
      phone sizes with the browser's device mode"), `docs/30` (event row, layout-only warning,
      portrait recipe as the full four-action block with the bar/panels-inside-both-reference-rects limit;
      the `SetUiVisible` rows and `docs/STATUS.md` are owned by `ui_set_visible.md`;
      note `portrait_reference` is written without `Some(`; the
      `ui_demo` portrait variant's on-screen hint mentions the browser's device mode),
      `docs/15` (validate codes + the presentation-only action list), `docs/dev/browser_tests.md`,
      crate `CLAUDE.md` (UI scale is presentation-only; the `Val::Px` / `scale_factor` rule).
- [ ] Re-scope the Group v2 and mobile entries in the backlog.

## Decisions from the remaining questions (Frank, 2026-10-11)

6. **Initial hidden state:** check during Phase 3 (needs the swap); if a flash is visible, Phase 3 adds
   `initially_hidden` on `Group` and leaves; no engine-side `orientation:` field.
7. **Initial orientation:** computed from the real `Window` size at each `scene.ready`; falls back to
   last-known/`landscape` only for a degenerate size (including a minimized 0x0 window).
8. **`data-ready` source:** set by `ironhold_web` (wasm32-gated, beside `sync_debug_state_to_dom`,
   `lib.rs:809`) unless the spike shows `update_debug_state` (`lib.rs:412-415`) runs ungated in
   normal play, in which case `play.html` may set it from `#debug-state`'s first content instead.

## Open questions

- None blocking. Phase 0 spike outcomes (flash visible or not; `update_debug_state` gating) pick the
  branch of decisions 6 and 8.

## Acceptance criteria

- Given a project with no `ui_scale` block, when the full `test_web.py` suite runs after each phase,
  then no existing baseline changes.
- Given `ui_scale: (reference: (1280.0, 720.0))`, when the window is 1920x1080, then `ui_scale` in debug
  state is 1.5 and a full-screen root `Group` anchors its rows to the new edges.
- Given a raw scale of 0.88 with `snap: 0.25`, then the applied scale is 0.75 (snap down, no overflow).
- Given 390x844 with no `portrait_reference`, then exactly one clamp warning names the window size, the
  needed scale and the floor; with `portrait_reference: (720.0, 1280.0)` the scale is ~0.5 and no warning fires.
- Given a portrait window, when it crosses the aspect threshold, then exactly one `window.orientation:*`
  `UiEvent` fires per crossing, it is re-sent after every `scene.ready`, `window_orientation` matches, and a
  `global_on` rule using `SetUiVisible` swaps the layouts with no click-blocking backdrop and no
  pause-menu side effects.
- Given a `window.orientation:Portrait` typo, then `ironhold validate` errors.
- Given a PC window that is minimized (reports 0x0) and then restored, then `UiScale` is never written,
  no `window.orientation:*` event fires, and the scale after restore equals the scale before minimize.
  (Bevy re-lays out every UI root itself at 0x0; that is outside our control and not asserted.)
- `SetUiVisible` acceptance (unknown ids, typo errors, panel/`ActionBar` rejection) lives in
  `ui_set_visible.md`; this plan only consumes the action.
- Given the `3rd_person_game_demo` retrofit in a 390x844 window with `portrait_reference: (720.0, 1280.0)`,
  then the shared `ActionBar` and the dialogue panel are fully on screen in both orientations (decision 9:
  they fit inside both reference rects).
- Given an orientation event and a `scene.ready` rule in the same frame, then the orientation rule's
  actions run after spawn and no `scene.ready` rule is required to set layout visibility.
- Given a scene load on a window whose real size is already valid, then `window_orientation` reflects
  that size (not a `landscape` guess) by the first `scene.ready`.
- Given a UI node whose `/ scale_factor` feeds `Val::Px` (split-viewport label, target readout), when
  `UiScale` is 0.5 or 2.0, then it lands at the same on-screen position as at 1.0.
- Given the web build, when the browser window is resized, then the canvas follows without a reload and
  the loading overlay still waits for the engine (`data-ready`), not just the canvas element.
- Given the touch context, when a button is tapped, then its action fires, or the failure is recorded in
  the backlog as a touch-input gap.
