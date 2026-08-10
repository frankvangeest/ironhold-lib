---
name: local-coop-demo-room-conventions
description: local_coop_demo's unwritten room conventions — linear portal chain (no deep-link), one color identity per room, room_hint must list every exit, 22px font with wrap-not-clip Label overflow and an 82-char verified ceiling
metadata:
  type: project
---

`assets/projects/local_coop_demo/` has grown to 11 rooms (`main`, `room2`–`room11`) with several
conventions that are enforced only by copy-paste, never written down anywhere a designer reads.

- **Linear portal chain, no deep-link.** `main -> room2 -> ... -> room10`; each room has exactly a
  back portal and a forward portal, both wired through `logic/rules.ron`
  (`entity.entered:portal_to_roomN -> LoadScene`). `play.html` accepts only `?project=`, **no
  `?scene=`** — so verifying the newest room means walking 9 portals. Cost grows every stage.
  The **return** portal always reuses the previous room's forward-portal prefab verbatim (same
  event name, same rules entry, no new prefab) — established room8→room9, repeated room9→room10.
- **Every room's exits must be listed in its top-left hint ladder.** Ideally in `room_hint`
  itself; when the line would get too long the project instead adds a sibling Label (room9's
  `room10_exit_hint`). When a new room is appended, the *previous* room's hints must be edited too.
- **One colour identity per room** — blue/amber/teal/rose/gold/green/violet/crimson (room2-9),
  **steel/silver-blue (room10)**, **lime (room11)**. Ground prefab + portal accent cylinder + scene
  `lighting` all share the tone. **Only white is still free** — the next room needs a new tone
  decision. **Always cross-check the hint text against the portal prefab's accent cylinder colour**
  — room10 gets this right ("Red portal -> room 9" vs `portal_to_room9`'s `(0.90, 0.15, 0.20)`),
  and so does room11 ("Steel portal -> room 10" vs `portal_to_room10`'s `(0.55, 0.60, 0.70)`).
- **Every room gets its own `scene.ready:roomN` Log rule** — room9's was missing and was added
  together with room10's in the player-model-v2 change; the gap is closed for all 11.
- **Label overflow is WRAP, not clip, and the UI font is 22px** (not 16 — `main.scene.ron:78-82`
  is the only place that says so): text wider than `size.0` wraps and the wrapped line **overflows
  past `size.1` into whatever Label sits below it**. So an over-long line garbles its *neighbour*,
  not itself. **Verified ceiling at `size: (900.0, 32.0)` is 82 chars** (room8's `room_hint`, the
  longest shipped line that has actually been playtested). Several rooms' comments cite a much more
  conservative "known-good 59-char line" and deliberately split lines to honour it. Treat >82 as
  unverified and require a browser check or a pre-emptive shortening.
- **Top-left hint ladder is `y = 20 + 44n`, `x = 20`, `900x32`** — confirmed across every room.
  room3, room9 **and room10** (since its `room11_exit_hint` was added) are tied as the most crowded
  at 6 rungs (y=20..240). room11 has 4. room6/room8 show two extra tiers for per-player captions:
  `y=316` and `y=676`, `600x32`, at `x=20` / `x=660`. Flag cumulative footprint once a room passes
  ~5 rungs.
- **Rung ordering is inconsistent**: room3/room5/room9 put `controls_hint` first, room10 puts
  `room_hint` first. room10's order (what room am I in → exits → controls → thesis → detail) reads
  best; worth standardizing on it. **room11 regresses** — it puts the generic movement
  `controls_hint` LAST, behind two feature-specific hints.
- **Per-viewport "P{n}" corner labels are automatic** in any split scene
  (`split_viewport_player_label_spawn_system`, top-right of each viewport, driven by
  `CameraTargets`+`PlayerIndex`) — a room does NOT need its own Labels to tell viewports apart.
  room6/room8's extra caption tiers are for input-scheme text, not identity.
- **Label `id`s follow player-facing vocabulary, not the feature slug** (`controls_hint`,
  `room_hint`, `targeting_hint`, `parity_hint`, `animation_gap_hint`, `join_prompt`).
  `gamepad_hardening_hint` (room3) is the one that leaked a plan filename into an asset id.
- **No room sets `show_player_nameplate`** — only `show_nameplates: true` (NPC/prop-facing). So
  every player prefab's authored `display_name` ("Player 1", "Player 2 (primitive)") is invisible
  in-game, and rooms rely on hint text to identify players instead.
- **Screenshot baselines are incomplete** — `screenshot_baselines/scenes/` has main, room2-5, room7
  only (no room6/8/9/10). Only `local_coop_demo_main.png` is referenced by `index.html`, so this is
  cosmetic, but don't assume a new room gets a baseline.

**How to apply:** when reviewing a new `local_coop_demo` room, check (1) the previous room's hints
were updated *and* the new exit line sits adjacent to `room_hint`, not several rungs away, (2) every
new/changed Label line is counted and is ≤82 chars, (3) the hint text names the *sibling room that
demonstrates the contrasting default*, since a portal chain makes A/B comparison expensive, (4) the
portal accent colour actually matches the colour word in the hint. Related:
[[split-switch-prefab-duplication]], [[local-coop-system]], [[primitive-player-fields]].
