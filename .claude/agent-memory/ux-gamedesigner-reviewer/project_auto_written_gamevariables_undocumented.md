---
name: auto-written-gamevariables-undocumented
description: Capability-written GameVariables (e.g. targeting's target_display/target_name/target_id) get documented only in core CLAUDE.md, never in docs/ — designers can't discover them
metadata:
  type: project
---

Capabilities that auto-write `GameVariables` a designer can bind to a Label tend to be documented only in `crates/ironhold_core/src/CLAUDE.md` (a Rust dev file designers never see and that this reviewer must exclude), and NOT in `docs/`.

Concrete instance: the targeting capability auto-writes `target_display` ("<prefab> <id>"), `target_name` (prefab key), and `target_id` (instance id) on every select/clear. As of the 3rd_person_game_demo targeting review these three keys appear ONLY in `crates/ironhold_core/src/CLAUDE.md:129`. They are absent from `docs/20_data_formats.md` and `docs/30_runtime_events_and_logic.md`.

**Why:** these variables are populated by the capability, not by any visible `SetVariable` action in rules.ron, so a designer reading the RON has no trail to follow. The `bind` field doc (docs/20_data_formats.md:391) explains the mechanism generically but never lists which keys exist.

**How to apply:** Whenever a feature mentions "the capability auto-writes a GameVariable a designer can bind," check that the exact key names are listed in `docs/` (not just core CLAUDE.md). The natural home is a small table near the `bind`/`format` Label fields in docs/20_data_formats.md, plus a mention in the targeting section of docs/30_runtime_events_and_logic.md. Flag as a 🔴 blocker — it makes the feature undiscoverable.
