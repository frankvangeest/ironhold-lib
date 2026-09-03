---
name: animation-two-stage-pipeline
description: Two cooperating animation systems (resolver + controller/graph-builder) and the npc_revive sentinel leak that produces benign-but-noisy fallback warnings
metadata:
  type: project
---

NPC/character animation runs as TWO cooperating systems, not one. Confusing them leads to wrong root-cause calls.

- `capabilities/animation_resolver.rs` — the SINGLE WRITER of `AnimationController.current`. Consumes `AnimationRequests.queue` (where `PlayAnimationOn`/`PlayAnimation` push clip names). Resolves override-id / semantic-alias / raw-clip-name, applies cancel_on_move + expiry, and handles the `stop_action` sentinel (clears active override when the queued cmd equals `active.stop_action`). Has a "4b" guard: once `graph_initialized && node_indices` populated, a clip absent from the graph is cleared → falls back to `base.idle` with WARN `animation_resolver.rs:193 "... not found in graph ... clearing override, falling back to idle"`.
- `capabilities/animation.rs` — builds the AnimationGraph from the merged GLB sources, finds the AnimationPlayer entity, and PLAYS `controller.current`. If `current` is not in `node_indices` it WARNs `animation.rs:235 "No node index for animation ... Resetting to idle"` and resets current to idle. Also owns the GLTF-respawn recovery (`animation.rs:171 "AnimationPlayer entity changed v0→v1"`).

**npc_revive sentinel leak — FIXED via a generalized `is_stop_sentinel` check.** The resolver (`animation_resolver.rs`, in the queued-command handling) now checks, for every incoming command, whether it matches **any** policy override's `stop_action` (`is_stop_sentinel = policy.overrides.iter().any(|d| d.stop_action.as_deref() == Some(cmd.as_str()))`) — not just whether a matching override is *currently active*. When it's a sentinel, the command clears the active override if one with a matching `stop_action` is active, and is `continue`d past regardless — so a sentinel command can never fall through to the raw-clip-name branch and become a chosen clip, even on a fresh spawn with no active death override. This closes the double-warning noise ("No node index ... Resetting to idle" + the resolver's own "not found in graph") this section previously described as benign-but-noisy. The fix generalizes to any `stop_action` sentinel, not just `npc_revive` specifically.
