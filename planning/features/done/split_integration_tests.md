# Feature: Split `integration_tests.rs` into domain files

_Status: Done_
_Planned at: `e788b74` (2026-07-02)_

## What

`crates/ironhold_core/tests/integration_tests.rs` had grown to 104 tests / 4258 lines mixing
8 distinct subsystems (global FSM, per-entity FSM/intent layer, scene lifecycle, spawn pipeline,
misc action-executor behaviors, NPC/camera, nameplates, UI) with no internal organization beyond
section-banner comments. Split it into 8 domain files, each independently compiled and run as
its own test binary, matching the existing convention already used by `audio_tests.rs` /
`stats_tests.rs` / `particle_tests.rs`.

## Why

Promoted from `planning/claude_suggestions.md` (originally observed at `9492ebf` 2026-05-27,
when the file was 2447 lines / 69 tests). By 2026-07-02 it had nearly doubled to 104 tests —
past the point where a related test could be found without searching. Every new feature's tests
defaulted into this one file by inertia; splitting it now (rather than waiting further) stops
that compounding.

## Approach (as shipped)

A one-off Python script parsed the file into discrete top-level items (tests + helper functions)
using a brace-depth heuristic — verified safe because every top-level Rust item in this
(rustfmt-formatted) file closes with an unindented `}` at column 0, so item boundaries could be
found reliably without a full Rust parser. Each `#[test]` item's leading contiguous `///`/`//`
comment block was captured and kept attached. The script verified 1:1 coverage (every one of the
112 items — 104 tests + 8 helper functions — assigned to exactly one output file, no drops, no
duplicates) before writing anything.

**Resulting files** (all under `crates/ironhold_core/tests/`):

| File | Domain | Tests |
|---|---|---|
| `fsm_tests.rs` | Global FSM: state transitions, rules matching, `ActionQueue` FIFO | 23 |
| `entity_logic_tests.rs` | Per-entity FSM, intent event layer | 8 |
| `scene_lifecycle_tests.rs` | Scene load/unload, overlays, model fixes, pipeline warmup | 18 |
| `spawn_tests.rs` | `Action::Spawn`/`Despawn`, spawn queue, preload, composite prefab | 15 |
| `action_tests.rs` | Misc action executor: variables, delayed events, floating text, target indicator | 13 |
| `npc_tests.rs` | NPC aggro/investigating, camera shake | 8 |
| `nameplate_tests.rs` | Nameplate anchor spawn, visibility filtering, cleanup | 10 |
| `ui_tests.rs` | Button click wiring, `IconButton` sync | 9 |

Each file got the full shared `use` header copied verbatim (simplest way to guarantee
compilation); `cargo fix --test <name> --allow-dirty` then mechanically removed the resulting
per-file unused imports, verified by re-running the full suite afterward.

A cross-domain helper (`npc_aggro_test_player_controller`, originally shared by NPC-aggro tests
and camera-shake tests) was resolved by moving both test groups into the same file (`npc_tests.rs`)
rather than duplicating the helper — camera shake is wired to player-hit events in this project
anyway, so the pairing is thematically reasonable, not just a technical convenience.

No production code changed — this is a test-only reorganization. No WASM build or play-test
applies.

## Tasks
- [x] Parse and verify 1:1 item coverage before writing any output file
- [x] Write the 8 domain files; delete `integration_tests.rs`
- [x] `cargo fix --test <name>` per file to drop unused imports
- [x] Full suite re-run: 104 passed, 0 failed (matches pre-split count exactly)
- [x] Update `crates/ironhold_core/tests/CLAUDE.md` file layout table
- [x] Update root `CLAUDE.md` build-commands section and the Critical Rules step 4 test gate
- [x] Update `.claude/commands/ship.md` step 6 test command
- [x] Update `.claude/agents/debug-detective.md` test-command references
- [x] Strike the `claude_suggestions.md` Testing entry

## Open questions
None — architect was not consulted (test-only, no schema/architecture surface), per the
project's own rule that a feature file may be skipped for simple additions; written anyway
because Frank asked to promote the suggestion to a tracked feature.

## Acceptance criteria
- `cargo test -p ironhold_core --test '*'` runs all 104 previously-`integration_tests.rs` tests
  (now spread across 8 binaries) plus every other test file, all green.
- No test name, assertion, or behavior changed — this is a pure file reorganization.
