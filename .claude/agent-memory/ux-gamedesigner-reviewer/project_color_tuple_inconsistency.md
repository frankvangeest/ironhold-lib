---
name: Color tuple shape varies across schema (RGB vs RGBA)
description: Designers face inconsistent color tuple arities — DamagePopupStyle uses 3-tuples while StatLabelDef/WorldStatBarDef use 4-tuples in the same prefab block
type: project
---

Color fields in newly added schema types are inconsistent in tuple length:

- `DamagePopupStyle.damage_color` / `heal_color` — `(f32, f32, f32)` (RGB, 3-tuple)
- `StatLabelDef.color` — `(f32, f32, f32, f32)` (RGBA, 4-tuple)
- `WorldStatBarDef.fill_color` / `bg_color` / `color_bands[].1` — `(f32, f32, f32, f32)` (RGBA, 4-tuple)
- Existing `EntityLabelDef.color` — `(f32, f32, f32, f32)` (RGBA, 4-tuple)

**Why:** This trips designers who copy a color literal between widgets. A 4-tuple in a 3-tuple slot (or vice versa) produces a RON parse error that doesn't clearly say "wrong arity for color".

**How to apply:** When reviewing new color fields, flag any deviation from the 4-tuple RGBA pattern that the rest of the codebase uses. If a 3-tuple is genuinely intended (because alpha would be meaningless), the doc table and inline example must call that out explicitly. Cross-check Rust docstrings against doc tables — the schema docstring for `damage_color` says "Linear RGBA" but the type is RGB; this kind of mismatch is a friction point.
