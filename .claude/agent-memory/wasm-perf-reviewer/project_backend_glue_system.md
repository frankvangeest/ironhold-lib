---
name: backend-glue-system
description: runtime/backend.rs (online backend v1): backend_glue_system per-frame Update cost, NonSend BackendSlot, alloc profile; reviewed 2026-10-27
metadata:
  type: project
---

backend_glue_system (Update, GameStreamSet::Backend) runs every frame for EVERY project, even offline ones.
Idle profile verified: `Vec::new()` batch + `filter().collect()` of expired ids allocate nothing when empty,
so idle frames are alloc-free (~tens of ns). Do not re-flag the batch Vec.
- `let hub = &mut *hub;` DerefMuts BackendHub every frame -> Changed every frame. No consumer of
  Changed<BackendHub> existed at review time; flag if one appears.
- BackendSlot is a NonSend resource (wasm adapter owns !Send JS handles): pins glue + loader to main thread
  on native only; free on WASM.
- load_backend_config_system is run_if(!Done): offline projects reach Done the frame ProjectConfig loads.
- Allocations are per reply/per action (format! event names, ~25 Strings per leaderboard list reply,
  is_busy() builds dedupe_key Strings per in-flight entry per dispatch) -- event-driven, fine.
- Replies are emitted while paused -> lands in the paused-Messages growth issue ([[real-pause]]), but
  volume is bounded by user-issued requests.
- Zero new deps; FakeBackend ships in release (needed for `fake://`). Size impact est. <50 KB.
**How to apply:** when the Nakama adapter lands, re-check poll() for per-frame JS interop/allocation
(e.g. draining a JS queue each frame) -- that is where real per-frame cost would appear.
