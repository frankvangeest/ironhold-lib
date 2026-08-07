---
name: ron-enum-double-paren
description: RON enum variants wrapping a named-field struct need DOUBLE parens — Orbit((field: value)) — a recurring copy-paste trap in docs examples
metadata:
  type: project
---

A RON enum newtype variant whose payload is a named-field struct needs **two** layers of parens:
`camera_mode: Orbit((offset: ..., ...))`. The single-paren form `Orbit(offset: ...)` fails with
`Expected struct CameraConfig but found "offset"`. Confirmed empirically 2026-08-07 during
`camera_modes.md` v1 (the implementer hit it and fixed it before ship).

This differs from variants whose payload is an inline anonymous struct — e.g. `JumpConfig`'s
`Fixed(height: 2.5)`, `Pixel(size: (60.0, 6.0))`, `ActionBar((owner_player: 0))` — where the
single/double distinction depends on whether the variant wraps a *named struct type* or declares
its fields inline. Designers cannot tell these apart by looking, and neither can a doc writer
working from the Rust type alone.

**Why:** this is the highest-frequency class of "docs example fails to parse if copy-pasted" in this
repo, and `ironhold_cli validate` is the only thing that catches it.

**How to apply:** whenever a new enum-with-struct-payload field is added to the RON schema, verify
every fenced example by actually running `ironhold_cli validate` against a project that uses it —
do not trust an example written from the schema. `docs/20_data_formats.md`'s "RON syntax gotcha"
callout under "Unified camera modes" is the model to copy: it states the wrong form, the exact
error text, and why. Reuse that pattern for the next such field.
