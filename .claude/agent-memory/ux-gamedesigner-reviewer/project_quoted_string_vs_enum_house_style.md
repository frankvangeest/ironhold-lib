---
name: quoted-string-vs-enum-house-style
description: The designer surface mixes quoted-string enums (orbit_button "Right") with real unquoted RON enums (velocity_curve EaseOut) — check which style a new field copies, and whether the CLI validates its string keys
metadata:
  type: project
---

This schema has **two incompatible ways to spell an enum-ish value in RON**, and both are shipped
and load-bearing. New fields must state which one they use, because a designer's muscle memory
from one produces a parse error in the other.

- **Quoted string, parsed at load:** `orbit_button: "Right"` / `"Either"` / `"None"`,
  `look_button: "Either"`, `character_rotate_button: Option<String>` (`Some("Right")`).
  Note `orbit_button: "None"` (a magic *string* meaning "disable mouse orbit") sits in the same
  `CameraConfig` block as `character_rotate_button: None` (an unquoted *Option*) — a live
  quoted-magic-string precedent, and a live footgun.
- **Real unquoted RON enum:** `velocity_curve: EaseOut` (Linear/EaseOut/EaseIn/**Pulse**),
  `emitter: Ring(radius: 1.0)`, `tonemapping: AcesFitted`, `split.orientation: Horizontal`,
  `axis: Y`, `style: Pixel`, `kind: Primitive`.

**Easing specifically:** `velocity_curve` (particles) already ships `Linear/EaseOut/EaseIn/Pulse`
as an unquoted enum. Any *new* easing field (e.g. a camera-transition `ease:`) must either reuse
that enum or explicitly document how its variant set differs — `EaseInOut` exists in neither, and
`Pulse` is meaningless for a camera blend.

**Companion rule — string keys need CLI validation.** `crates/ironhold_cli/src/commands/
validate.rs` already cross-checks action string keys against their catalogs (effect, decal, audio,
prefab, model, modifier, `entities[].prefab`, `join_prefab_keys[]`). Any new action carrying a
free-form `String` key into a named registry should join that list in the same pass, otherwise a
typo is an unreported silent no-op — the single most common designer error class.

**How to apply:** on any new enum-shaped or string-keyed field, ask (a) quoted or unquoted, and is
that consistent with the nearest sibling field a designer will have just copied? (b) is there a
reserved/magic value (`"None"`, `"default"`, `"player"`, `{self}`) and is it documented *at the
field*, not only in prose? (c) does `ironhold_cli validate` reject a typo in the key?

Related: [[schema-bool-toggle-house-style]] (bool vs enum for binary toggles),
[[ron-enum-double-paren]] (double-paren rule for variants wrapping a named struct),
[[camera-config-party-split-nesting]].
