---
name: ocean-demo-plan-review
description: 2026-10-02 plan-review of ocean_simulation_demo.md (8 phases) - angle-convention clash, SetParam key naming rule, boat input vs InputMap, readout formatting; recheck these when any phase is implemented
metadata:
  type: project
---

Plan-review verdict 2026-10-02: Needs more design work, 4 blockers. Re-check each when the matching phase lands:

1. **Angle conventions** - plan uses compass (0=-Z, 90=+X, clockwise) for waves/wind/sun, but
   entity `rotation_euler_deg` Y is (standard right-handed) counter-clockwise and its handedness
   is undocumented in docs/20; sun is authored as `directional.rotation_euler_deg` yet plan adds
   `sun.yaw_deg/pitch_deg` keys. Recommended: `toward_deg` naming, sun `azimuth_deg/elevation_deg`,
   a worked conversion example. Wind "toward" contradicts the meteorological "from" convention.
2. **SetParam keys must mirror RON field paths** (plan had `ocean.spec.*`, `sun.*`,
   `ambient.brightness`, `boat:<id>.x` - none match schema; stats precedent is `"spawn_id.field"`).
   Want a full key table (unit/range/default), `query params`, validate on {self} keys + slider ranges.
3. **Boat input** - plan put keys in `boat.input` instead of `components.inputs` (InputMap +
   gamepad_index); invented gamepad axis names; throttle hold-vs-ratchet and script-vs-keyboard
   precedence undefined; no vessel-player field-applicability list.
4. **Readouts** - GameVariables are strings, Label has no precision control -> raw f32 jitter;
   `boat.speed_mps` not per-boat. Recommended Label `decimals`, slider `format:` not `label:`.

Other: v6 should split (fog+light keys are useful to every project, no ocean dep); Slider should
support a `variable:` target; demo needs a `main.scene.ron` for the gallery card; every ocean scene
belongs in `NON_DETERMINISTIC_SCENES`. Related: [[docs-lag-actions]], [[ron-enum-double-paren]],
[[schema-bool-toggle-house-style]], [[pause-is-cosmetic]], [[ui-label-button-font-and-clip]].
