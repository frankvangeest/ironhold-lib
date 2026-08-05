---
name: local-coop-demo-room-conventions
description: local_coop_demo's unwritten room conventions — linear portal chain (no deep-link), one color identity per room, room_hint must list every exit, ~70-char Label budget at 900x32
metadata:
  type: project
---

`assets/projects/local_coop_demo/` has grown to 9 rooms (`main`, `room2`–`room9`) with several
conventions that are enforced only by copy-paste, never written down anywhere a designer reads.

- **Linear portal chain, no deep-link.** `main -> room2 -> ... -> room9`; each room has exactly a
  back portal and a forward portal, both wired through `logic/rules.ron`
  (`entity.entered:portal_to_roomN -> LoadScene`). `play.html` accepts only `?project=`, **no
  `?scene=`** — so verifying the newest room means walking 8 portals. Cost grows every stage.
- **Every room's `room_hint` Label lists both exits by portal colour** (e.g. "Room 3: vertical
  split | Blue portal -> room 2 | Cyan portal -> room 4"). When a new room is appended, the
  *previous* room's `room_hint` must be edited too — this was missed for room8 -> room9
  (2026-07-29), leaving the new portal discoverable only from its 3D world label.
- **One colour identity per room** (blue/amber/teal/rose/gold/green/violet/crimson taken through
  room9) — ground prefab + portal accent share the tone. **The palette is now exhausted**; a 10th
  room needs a new tone chosen deliberately (steel/silver, lime, and white are still free).
- **Every room gets its own `scene.ready:roomN` Log rule in `logic/rules.ron`** — main..room8 all
  have one; **room9 does not** (missed 2026-07-29, same oversight shape as the room_hint gap). Add
  the missing one whenever the next room is appended.
- **Label width budget:** `size: (900.0, 32.0)` at the default `font_size: 16.0`. room3's own
  comment cites a "known-good 59-char line"; the longest shipped line is room8's `room_hint` at
  93 chars. Anything past ~90 chars is unverified and risks clipping the one line carrying the
  feature's explanation.
- **Top-left hint ladder is `y = 20 + 44n`, `x = 20`, `900x32`** — confirmed across every room.
  room3 is the most crowded at 6 rungs (y=20..240, block ends at y=272 ≈ 38% of a 720px window);
  room9 has 5, most rooms 2-3. room6/room8 show two additional tiers exist for per-player captions:
  `y=316` and `y=676`, `600x32`, paired at `x=20` / `x=660`. Each new feature appends a rung to
  room3 rather than replacing one — flag the cumulative footprint, not just the individual label,
  once a room passes ~5 rungs.
- **Label `id`s follow player-facing vocabulary, not the feature slug** (`controls_hint`,
  `room_hint`, `targeting_hint`, `ability_hint`, `join_prompt`). `gamepad_hardening_hint` (room3,
  2026-08-01) is the one that leaked a plan filename into an asset id — don't set that precedent.
- **Screenshot baselines are incomplete** — `screenshot_baselines/scenes/` has main, room2-5,
  room7 only (no room6/room8/room9). Only `local_coop_demo_main.png` is referenced by
  `index.html`, so this is cosmetic, but don't assume a new room gets a baseline.

**How to apply:** when reviewing a new `local_coop_demo` room, check (1) the previous room's
`room_hint` was updated, (2) the new room's own hints stay under ~90 chars, (3) the room's UI text
names the *sibling room that demonstrates the contrasting default*, since a portal chain makes A/B
comparison expensive. Related: [[split-switch-prefab-duplication]], [[local-coop-system]].
