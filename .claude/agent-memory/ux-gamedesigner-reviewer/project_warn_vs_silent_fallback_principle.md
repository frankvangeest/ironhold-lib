---
name: warn-vs-silent-fallback-principle
description: When Ironhold chooses to warn! vs silently fall back on a designer authoring omission — the discriminating principle and the precedents on each side
metadata:
  type: project
---

Ironhold's de-facto rule for whether a designer authoring gap produces a `warn!` (and often a paired `ironhold_cli validate` error) or a silent fallback:

**Warn when the designer's authored intent is ambiguous or self-contradictory** (config that can't do what it evidently sets out to do). **Stay silent when the fallback is a legitimate, common authoring choice.**

Precedents that WARN (intent ambiguous/contradictory):
- `warn_cross_bar_duplicate_keys` (scene_loader.rs) — two bars share a slot key; also a cli `cross_bar_duplicate_key` error. Fires at SCENE LOAD, not per-frame.
- Duplicate `player_index` across players (entity_spawner.rs ~line 534) — two "primary" players.
- Missing `animation_policy` on a player prefab (`assemble_player_config`, entity_spawner.rs:939-942) — a player-authoring omission that plainly wasn't intended.
- Camera: `party`+`split` both set (split wins + warn); neither set (falls back to single OrbitCamera + warn).
- `stat_overrides` unknown key (docs 20_data_formats.md:254) — warns.
- `warn_missing_player_stat_templates` (scene_loader.rs ~1340) — **the canonical scene↔prefab
  cross-file precedent**: a bar's slot `cost.stat` isn't in the owning player prefab's
  `stat_templates`. Proves a scene-UI field CAN be cross-checked against the owning player's
  prefab at load time. Cite this whenever a new `owner_player`-scoped field silently depends on a
  prefab-side field (e.g. `gamepad_key` depending on `InputMap.gamepad_index`).
- `warn_same_player_gamepad_duplicate_slots` (scene_loader.rs ~1260) — same player, 2 slots, same
  `gamepad_key`; paired cli `same_player_gamepad_duplicate_key` error. Keyed by
  `(owner_player.unwrap_or(0), button)`.

Precedents that stay SILENT (legitimate common choice):
- Nameplate/`world_stat_bar` stat bar for a stat the entity lacks — "silently skipped, no error" (docs:642). A skeleton with no mana bar is normal.
- Player using global `player_health` silently failing to match `{self}.*` (docs:618-624) — documented, not warned.
- A player prefab declaring NO `stat_templates` at all (ordinary global-pool fallback).

Known GAP where the principle says warn but nothing does (flag on any gamepad/action-bar review):
- `ActionSlotDef.gamepad_key` set while the owning player has **no** `InputMap.gamepad_index` —
  the binding is silently inert (`resolve_gamepad(None)` → `None`); no warn, no validate error, undocumented.
- `gamepad_key` colliding with the owning player's own `gamepad_jump`/`run`/`interact`/`target_next`
  (all four default to South/East/West/North) — one press does both; entirely unchecked.

- Flycam prefab with non-empty `model`/`children`, and dual `tags: ["player","flycam"]`
  (scene_loader.rs `is_flycam` branch, shipped 2026-08-19) — paired cli
  `flycam_model_never_renders` / `flycam_player_tag_conflict` hard errors. See
  [[flycam-spectator-priority]] for the still-uncovered `shape:`/`primitive:` variant.

**Message-content rule (separate from the warn/silent decision):** a diagnostic must end with the
*remedy*, not just the diagnosis. The house precedent is the duplicate-flycam docs note — "Delete
all but one to silence it." Any new `warn!`/validate error should name (a) the offending
entity/prefab/field keys, (b) what will not happen, (c) the exact edit that silences it, and (d) the
alternative authoring path if the designer's evident intent is achievable another way. Plans that
only specify "name the ids and state it's by-design" are incomplete — "this is by design" reads as
the engine excusing itself, not as instructions.

**Corollary (learned on the flycam diagnostic, 2026-08-19): when a diagnostic covers 2+ possible
offending fields, the remedy clause must be derived from the fields actually detected, not a single
fixed string covering all of them.** A message that always says `Set model: "" (and/or remove
children)` tells a designer whose only mistake was `children:` to make an edit they've already made
(`model: ""` is the Primitive convention) — reads as a broken/generic error. Branch the remedy on
the detected field list, and add a test fixture per branch so each wording is actually exercised.

**How to apply:** When reviewing a new fallback path, recommend WARN if the fallback contradicts an evident per-entity/per-player intent signal (e.g. `owner_player` set, 2+ players, a partial `stat_templates` block present but missing the referenced key). Recommend SILENT if the fallback is the ordinary single-player/global case. Always specify TIMING: prefer scene-load or cli-validate time (follows `warn_cross_bar_duplicate_keys`) over per-frame/per-press, which spams the log.
