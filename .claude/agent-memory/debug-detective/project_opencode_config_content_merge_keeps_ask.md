---
name: opencode-config-content-merge-keeps-ask
description: OPENCODE_CONFIG_CONTENT deep-merges per key, so overriding bash "*" to deny leaves the repo's explicit "ask" entries (git push, cargo clean, worktree remove, external_directory "*") which headless runs auto-approve (V20)
metadata:
  type: project
---

Overriding only `permission.bash["*"] = "deny"` (or `task["*-deep"]`) through `OPENCODE_CONFIG_CONTENT` does NOT make an unattended `opencode run` safe: the merge keeps every other key of the repo's `.opencode/opencode.json`, including explicit `"ask"` entries (`git push*`, `git merge*`, `cargo clean*`, `git worktree remove*`, `find * -exec*`) and `external_directory "*": "ask"`, and fact V20 says headless runs auto-approve `ask`. Also kept: allow entries like `python -c*`, `cargo build*`, `wasm-pack build*` (arbitrary code, shared CARGO_TARGET_DIR concurrency hazard).

**Why:** found in the opencode_glm_driver_pilot step-4 review (2026-10-09): `tools/opencode_pilot.py build_config` claimed "bash deny-by-default" but only flipped `*`.

**How to apply:** when reviewing any unattended/headless OpenCode runner, enumerate every `ask` value in the merged permission tree (bash, external_directory, task, edit) and require each be rewritten to `deny`; also check allow entries that escape the throwaway worktree. Related: [[opencode-probe-load-edge-facts]].
