---
name: Docs lag the action schema
description: Designer-facing docs (20_data_formats.md actions table, 30_runtime_events_and_logic.md Appendix, STATUS.md ABI list, 60_contributing.md validate checks list) are consistently not updated when new Action variants/fields land in schema/actions.rs — check 5 surfaces, not just 1
type: project
---

When new `Action` variants — **or new optional fields on an existing variant** — land in
`crates/ironhold_core/src/schema/actions.rs`, five doc surfaces need checking. Surface 1 usually
gets updated; the other four consistently lag:

1. `docs/20_data_formats.md` — the "Available actions" table (under `## logic/rules.ron —
   LogicRulesAsset`). **This one usually DOES get updated.**
2. `docs/30_runtime_events_and_logic.md` — the "Implementation snapshot" action bullets, the
   `#### Animation/audio actions` / `## Action model` category lists, AND the `### Actions ✅`
   appendix.
3. `docs/STATUS.md` — the `Engine ABI` actions list.
4. `docs/STATUS.md` — the `### Capabilities` table row for the affected area (e.g. an "Animation
   playback" row still reading a stale one-line summary after the real feature shipped).
5. `docs/60_contributing.md` — the `validate <project_dir>` ▸ **"Checks performed"** list and its
   `--strict` counterpart, whenever the change also adds an `ironhold_cli validate` error. House
   style there: name the `error_type` string in backticks and cross-link to the relevant docs/20
   section. This is empirically the single most-missed surface for `validate`-adding changes — the
   feature plan's own Docs task line often only names docs/20 and docs/30, baking the omission in
   at plan time. See [[validate-coverage-gaps]].

There is an explicit reminder in `30_runtime_events_and_logic.md` at the end of the appendix:
> "New Messages or Actions must update `docs/STATUS.md` (Engine ABI section), this appendix, and `docs/20_data_formats.md` with an authoring example."

This is regularly ignored in practice, historically for both new Action variants (e.g.
`ShowFloatingText`, `PlayAnimationOn` gaining new optional fields) and new schema surfaces more
broadly: `PrefabDef` fields (e.g. `stat_label`/`world_stat_bar`, `NpcDef.collider_radius`/
`collider_height`), and entire new prefab `kind`s (e.g. `Foliage` shipping with zero
`docs/20_data_formats.md` entries). **Contrast:** schema *fields* and *events* tend to get
documented correctly even when new struct-variant *actions* get missed — the lag is
action-table-specific, not feature-wide. Also watch for accuracy drift, not just omission: a
documented action can describe stale behavior (e.g. singular-camera language after a feature made
it apply to multiple cameras in a split-screen scene) when the underlying mechanism changes without
a doc pass.

**Why:** the schema is the source of truth (Rust); designers only see the docs. A new Action that
exists only in Rust + an example RON file is essentially un-discoverable for a designer building a
new project from scratch.

**How to apply:** when reviewing any new Action variant, new optional field on an existing Action,
or new `PrefabDef`/prefab-`kind` schema surface, check all 5 doc surfaces above (not just docs/20)
and flag missing entries as blockers. Also flag missing entries on the `{self}` substitution list
in `crates/ironhold_core/src/CLAUDE.md` (developer-side, not designer-side, but the project-internal
reference for what `{self}` does).
