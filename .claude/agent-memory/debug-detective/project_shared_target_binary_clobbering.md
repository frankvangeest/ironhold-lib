---
name: shared-target-binary-clobbering
description: The shared CARGO_TARGET_DIR debug binary can be rebuilt out from under a probe session by another worktree, silently turning real findings into fake false-negatives
metadata:
  type: project
---

Root CLAUDE.md forbids concurrent cargo across worktrees, but the *debugging* consequence is
worse than a corrupt build: `$CARGO_TARGET_DIR/debug/ironhold.exe` is a single shared file, so
another worktree's `cargo build/test/check` replaces the binary you are probing with one built
from *its* branch. Symptom (real, 2026-09-07): a set of CLI fixtures that had just reproduced 3
hard errors began reporting `exit 0, all valid` on byte-identical inputs, and the official test
fixture for a brand-new check also went green — looking exactly like a nondeterminism bug in the
check under test.

**Why:** the binary's mtime/size is the only tell, and nothing in the CLI output identifies which
source built it.

**How to apply:** for any probe session driving `ironhold.exe` (or any built binary) repeatedly:
1. `cargo build -p ironhold_cli`, then immediately **copy the binary into the scratchpad** and run
   every probe against that private copy — never against `$CARGO_TARGET_DIR/debug/` directly.
2. Before believing a "the check didn't fire" result, run one *known-positive* fixture as a
   liveness control (a green known-positive means your binary is wrong, not the check).
3. `md5sum` the source file before and after the build, and again after the probe run — a
   feature worktree can also be edited concurrently by the implementing agent mid-review, which
   makes any finding meaningless unless it's pinned to a hash.

Related: [[stale-cli-binary-as-prefix-oracle]] — that note uses a *deliberately* stale
`tools/bin/ironhold.exe` as a pre-fix oracle; this is the same mechanism firing accidentally.
