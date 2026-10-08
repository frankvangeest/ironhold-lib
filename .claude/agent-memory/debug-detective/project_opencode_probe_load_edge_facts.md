---
name: opencode-probe-load-edge-facts
description: Probe-verified OpenCode 1.18.33 read-tool attach edge cases on Windows (string-prefix containment, lowercase agents.md, nested agent worktrees, --params quoting, repo-wide session list) that tools/opencode_probe.py and the .opencode/README.md facts table got wrong or missed
metadata:
  type: project
---

Verified 2026-10-08 with `opencode debug agent build --tool read --params ...` (model-free, XDG_DATA_HOME/XDG_STATE_HOME pointed at a temp dir) while reviewing feature/opencode_compat_probe.

- **Out-of-project attach uses STRING-PREFIX containment.** From the primary checkout `...\ironhold-lib`, reading `../ironhold-lib-<slug>/crates/.../lib.rs` attaches that worktree's folder CLAUDE.md files plus its root AGENTS.md. The reverse direction (worktree reading `../ironhold-lib/...`), or any unrelated cwd, attaches NOTHING. So facts-table row V14 holds only because of the `ironhold-lib-{slug}` naming convention. `expected_attach` in the probe has the same `startswith(root_n)` guard, so the two agree by coincidence.
- **On Windows, a lowercase `agents.md` is picked up as AGENTS.md** and shadows CLAUDE.md. `check_no_subfolder_instruction_files` uses a case-sensitive `name in filenames`, so it misses it, and the probe reports PASS because its expectation also resolves AGENTS.md. No check fires at all.
- **A nested `.claude/worktrees/agent-x/` attaches its own `AGENTS.md`** (it is tracked at the root on main), not its CLAUDE.md. Row V13 says "loads its own root CLAUDE.md", which is wrong. The static scan skips `.claude/worktrees`, so this never shows up.
- **`--params` accepts real JSON** through a subprocess argv list with the real `opencode.exe` (`json.dumps({"filePath": p, "limit": 2})`). The JS single-quote literal the probe builds breaks on any path that contains `'` (exit 2, "Failed to parse --params").
- **`opencode session list` is repo-wide across worktrees.** A worktree created that day listed sessions from weeks earlier. Deleting by title prefix therefore reaches every worktree's sessions.
- A missing file makes debug exit 1 with "File not found", which is a tool error and not a false pass. A directory read returns `loaded: []`.

**Why:** these break the probe's assumptions without any visible error.
**How to apply:** when reviewing changes to the OpenCode tooling or the facts table, re-test these edge cases with the raw debug command. Related: [[opencode-agents-md-shadows-claude-md]], [[shared-target-binary-clobbering]] (another session may edit the same worktree concurrently, so diff the working tree before reporting).
