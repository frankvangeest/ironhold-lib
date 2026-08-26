---
name: serve-py-stale-checkout-trap
description: A long-lived serve.py serves its start-time cwd, so a browser playtest of a feature worktree silently exercises the primary integration checkout instead — verify by comparing HTTP byte counts against both dirs
metadata:
  type: project
---

`serve.py` is `http.server.SimpleHTTPRequestHandler` with **no `directory=` argument**, so it
serves whatever the process's cwd was **when it started**. A `serve.py` left running from the
primary checkout (`.../Ironhold/ironhold-lib`, permanently on `integration`) keeps serving
`integration`'s assets and `pkg/` even after a `feature/*` worktree is created and built — the
browser playtest then exercises code and RON that **does not contain the feature at all**.

**Why:** confirmed root cause of a reported "monster corpse loot doesn't open" bug (2026-08-24).
The whole pipeline was correct; the served `enemy_zombie.behavior.ron` was the pre-feature version
(single `dead` state, no `entity.interacted:{self}` handler) and the served `prefabs.ron` had no
`interactable`/`inventory` on the zombie. Symptom of a stale-asset playtest is
indistinguishable from a real bug: the interact key simply does nothing, warn-free — the entity
has no `Interactable`, so `interactable_system` emits only `player.attack_missed`.

**How to apply:** for ANY browser-playtest bug report on a feature branch, check the server before
reading code. Two cheap, decisive checks:

1. Process start time vs. worktree creation time — a `serve.py` older than the worktree cannot be
   serving it:
   `Get-CimInstance Win32_Process -Filter "Name like '%python%'" | Select ProcessId, CommandLine, CreationDate`
   plus `git worktree list` and `stat -c %z <worktree>/.git`.
2. Byte-count triangulation — fetch the file over HTTP and compare `size_download` against
   `stat -c%s` in *both* checkouts. An exact match with the primary checkout is proof:
   `curl -s -o /dev/null -w "%{size_download}" http://localhost:8000/<path>`
   Do this for a changed RON file **and** `pkg/ironhold_web_bg.wasm` (a `--dev` build is ~146 MB
   vs a `--features webgpu` release ~98 MB, so the sizes alone often identify which build is live).

RON is fetched at runtime, so a *stale wasm alone* cannot explain a missing RON-driven behavior —
if the symptom is "authored RON behavior absent", the served **assets** are stale, i.e. wrong
directory, not merely a missing rebuild. Related: [[project_browser_pixel_probe_recipe]].
