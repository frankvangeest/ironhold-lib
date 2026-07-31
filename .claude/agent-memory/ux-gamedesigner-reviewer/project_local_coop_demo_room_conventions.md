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
- **One colour identity per room** (blue/amber/teal/rose/gold/green/violet/crimson taken) —
  ground prefab + portal accent share the tone.
- **Label width budget:** `size: (900.0, 32.0)` at the default `font_size: 16.0`. room3's own
  comment cites a "known-good 59-char line"; the longest shipped line is room8's `room_hint` at
  93 chars. Anything past ~90 chars is unverified and risks clipping the one line carrying the
  feature's explanation.
- **Screenshot baselines are incomplete** — `screenshot_baselines/scenes/` has main, room2-5,
  room7 only (no room6/room8/room9). Only `local_coop_demo_main.png` is referenced by
  `index.html`, so this is cosmetic, but don't assume a new room gets a baseline.

**How to apply:** when reviewing a new `local_coop_demo` room, check (1) the previous room's
`room_hint` was updated, (2) the new room's own hints stay under ~90 chars, (3) the room's UI text
names the *sibling room that demonstrates the contrasting default*, since a portal chain makes A/B
comparison expensive. Related: [[split-switch-prefab-duplication]], [[local-coop-system]].
