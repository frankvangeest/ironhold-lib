---
name: project-despawn-fade-design
description: SetDespawnTimer fade_secs/fade_mode (planned 2026-10-02) — additive timing trap, Shrink fallback visibility, target-ring contradiction; check these when the feature ships
metadata:
  type: project
---

Plan `planning/features/fade_out_despawn.md` (reviewed 2026-10-02, pre-code) adds `fade_secs` (default 0) + `fade_mode: Fade|Shrink` (default Fade) to `SetDespawnTimer`.

**Why it matters:** timing is ADDITIVE (on-screen total = delay_secs + fade_secs) — adding fade to an existing timer silently lengthens it; fallbacks to Shrink (custom/premultiplied material, 16-entity budget) are only a browser-console warn!; plan text said held target clears only at despawn while its playtest expected no lingering target ring.

**How to apply:** when reviewing the implementation diff, verify docs/20 row + docs/30 corpse block (~757/765) state the additive rule, the 3 fallback triggers, "Despawn stays instant -> use delay 0 + fade N", colliders stay solid, re-arm during fade ignored; and that target clears at fade start. Demo users: lootable_corpse (both states), seal_door (state_machine.ron ~192, suggested second showcase); enemy_* swap timers must stay instant. Related: [[project-warn-vs-silent-fallback-principle]], [[project-floating-text-tracks-entity]].
