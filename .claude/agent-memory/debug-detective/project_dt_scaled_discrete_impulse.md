---
name: dt-scaled-discrete-impulse
description: Discrete input events multiplied by time.delta_secs() jump up to 15x on a hitch frame (Bevy Time<Virtual> max_delta = 250ms), and fixed-16ms tests can never see it
metadata:
  type: project
---

Pattern: an event-driven impulse (mouse-wheel notch, button press) applied as
`state -= delta * speed * time.delta_secs()`. Two compounding hazards:

1. **dt is not 16ms.** Bevy's `Time<Virtual>` clamps `delta` to `max_delta`, default **250ms** — so
   the applied amount varies by up to ~15x between a smooth frame and a hitch frame (WASM asset
   load, pipeline compile, GC). In a WASM release build hitches are routine.
2. **Batched events + inflated dt multiply together.** The same hitch frame that gives dt=0.25 also
   batches N input events into one `MessageReader::read()`. A per-*event* bound does not bound the
   frame total, so `N × 0.25` can exceed a whole clamped range even though each event looks sane.

**Why:** found while reviewing the scroll-wheel zoom normalization fix (`capabilities/camera.rs`
`normalized_wheel_delta`): per-event clamping to ±1.0 leaves the frame sum unbounded, and the
regression tests all pin dt to exactly `Duration::from_millis(16)`, so no test can observe either
hazard. This is the general reason "it only snaps sometimes / on my machine" survives a fix that
looks arithmetically correct.

**How to apply:** for a discrete impulse, prefer a fixed per-event step with **no** `delta_secs()`
multiply (dt-scaling an impulse is framerate-dependent by construction), and bound the per-frame
total, not just the per-event value. When reviewing regression tests for such a fix, treat a single
hard-coded dt as a coverage gap and ask for a large-dt case. Related:
[[mousewheel-unit-platform-reality]].
