# Feature: Fix Washed-Out White Icons

_Status: Draft_
_Planned at: `1932139` (2026-06-22)_

## What

White-on-transparent icons in the action bar and inventory panels look pale and hard to read.
This is a perception issue — not a code bug — caused by linear color authoring intuition and
low contrast against the slot background. Fix it with a documentation clarification and an
optional default slot-background tweak. No shader, no schema change.

## Why

Designers currently type `icon_color: (0.5, 0.5, 0.5, 1.0)` expecting a mid-gray tint but get
a much brighter result because all icon colors are interpreted as **linear** RGBA, not sRGB.
Linear 0.5 is visually closer to sRGB 0.73. This is the root cause of the "washed out"
perception. Fixing the documentation removes the guesswork and lets designers author effective
tints immediately. A contrasting slot background is the second lever — a slightly darker slot
interior makes even a pure-white untinted icon read clearly.

## Diagnosis (verified)

`icon_color` is applied via `ImageNode.color` (multiplicative tint) in three places:
- ActionBar slot — `scene_loader.rs:1812`
- Inventory slot — `scene_loader.rs:2023` spawned white, updated per-item in `inventory.rs:270`
- Container slot — `inventory.rs:336`

For a white-on-transparent icon `(1,1,1,a)`, `tint × (1,1,1,a) = (tint.rgb, tint.a × a)` —
color math is correct. No alpha-compositing bug. The visual washout comes from:

1. **Linear vs. sRGB intuition gap.** `Color::linear_rgba(0.5, 0.5, 0.5, 1.0)` is much brighter
   than designers expect when thinking in sRGB/perceptual gray. Nothing in the current schema docs
   explains this.
2. **Low contrast on light slot backgrounds.** Untinted white icons against mid-gray slot
   interiors have near-zero contrast, reading as a washed blob.

There is no premultiply bug — Bevy's UI pipeline handles that correctly.

## Approach

### Step 1 — Documentation (required)

In `docs/20_data_formats.md`, add a callout box to the icon color fields section:

> **Icon colors are linear RGBA, not sRGB.**
> A value of `(0.5, 0.5, 0.5, 1.0)` is perceptually about 73% brightness — much brighter than
> the sRGB "mid gray" (which is `(0.22, 0.22, 0.22, 1.0)` in linear). Use approximately
> `(0.22, 0.22, 0.22, 1.0)` for a visually neutral gray tint.
> White-on-transparent icons without a tint (`icon_color` omitted) will have low contrast against
> light slot backgrounds — use a darker slot `background_color`, or apply a slight tint such as
> `(0.85, 0.85, 0.85, 1.0)`.

### Step 2 — Default slot background review (optional tweak)

Review the default `background_color` values on action bar slots and inventory slots:
- Action bar slot default: `(0.18, 0.18, 0.22, 0.85)` — already fairly dark; may be sufficient.
- Inventory slot default: check current value in `scene_loader.rs`.

If the inventory slot background is too light, darken it slightly (e.g. to `(0.12, 0.12, 0.15, 0.90)`). This is a designer-quality-of-life change, not a correctness fix. The designer can always override it via RON.

## Tasks

- [ ] Add linear-RGBA callout to `docs/20_data_formats.md` under the `icon_color` field descriptions for `ActionSlotDef` and `ItemDef`.
- [ ] Review default slot background colors in `scene_loader.rs` for action bar and inventory; darken if needed.
- [ ] Play-test with the demo project's white icon sheet to confirm the icons read clearly at the new defaults.
- [ ] Update `crates/ironhold_core/src/CLAUDE.md` if there is any note about icon tinting behavior.

## Out of scope

- Switching `icon_color` to sRGB parsing (schema-semantics change; only if this fix proves insufficient).
- Custom `UiMaterial` shader for icons — see `planning/features/icon_three_channel_mask.md`.

## Acceptance criteria

- `docs/20_data_formats.md` contains a clear note that icon colors are linear RGBA with a concrete sRGB-equivalent example.
- White-on-transparent icons in `3rd_person_game_demo` are clearly readable against the slot background in the browser build.
- No RON files need changing as part of this fix.
