---
name: animation-two-stage-pipeline
description: Two cooperating animation systems (resolver + controller/graph-builder) and the npc_revive sentinel leak that produces benign-but-noisy fallback warnings
metadata:
  type: project
---

NPC/character animation runs as TWO cooperating systems, not one. Confusing them leads to wrong root-cause calls.

- `capabilities/animation_resolver.rs` — the SINGLE WRITER of `AnimationController.current`. Consumes `AnimationRequests.queue` (where `PlayAnimationOn`/`PlayAnimation` push clip names). Resolves override-id / semantic-alias / raw-clip-name, applies cancel_on_move + expiry, and handles the `stop_action` sentinel (clears active override when the queued cmd equals `active.stop_action`). Has a "4b" guard: once `graph_initialized && node_indices` populated, a clip absent from the graph is cleared → falls back to `base.idle` with WARN `animation_resolver.rs:193 "... not found in graph ... clearing override, falling back to idle"`.
- `capabilities/animation.rs` — builds the AnimationGraph from the merged GLB sources, finds the AnimationPlayer entity, and PLAYS `controller.current`. If `current` is not in `node_indices` it WARNs `animation.rs:235 "No node index for animation ... Resetting to idle"` and resets current to idle. Also owns the GLTF-respawn recovery (`animation.rs:171 "AnimationPlayer entity changed v0→v1"`).

**npc_revive sentinel leak (known benign noise, candidate for cleanup):**
The `stop_action: "npc_revive"` is declared on the death override in snake/spider/zombie policies. Enemy behaviors fire `PlayAnimationOn(clip: "npc_revive")` on the `alive` entry to clear a lingering death pose. BUT the stop_action only cancels when a death override is ALREADY active (resolver line ~118). On a FRESH spawn there is no active death override, so `npc_revive` falls through to the resolver's raw-clip-name branch and becomes the chosen clip. Before the graph is initialized the 4b guard can't catch it, so it reaches `animation.rs` as the starting clip → "No node index ... Resetting to idle". Once the graph initializes the resolver also emits its own "not found in graph" warning. Net effect is correct (idle), but produces 2 layers of warnings per enemy per spawn. Fix direction: make `npc_revive` (and any stop-action sentinel) a recognized no-op in the resolver's raw-clip-name branch when no matching active override exists — never let a sentinel become a chosen clip. Could also be done by reserving sentinel names in a policy field rather than overloading the clip namespace.
