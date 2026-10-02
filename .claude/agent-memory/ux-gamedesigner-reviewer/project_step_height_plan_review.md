---
name: step-height-plan-review
description: step_offset_auto_step plan-review (2026-10-02) - stair-tread rule unmeasured (~2x radius likely), player_p1_split_ring shared room9/room10, blank_project is the starter template to seed opt-in fields
metadata:
  type: project
---

Plan-reviewed `planning/features/step_offset_auto_step.md` (MovementConfig.step_height, default 0.0, players only) on 2026-10-02.
Verdict: Needs more design work, 2 blockers:
1. Docs tasks put gate rules (airborne/rooted/players-only/climbs small dynamic props+NPCs) in CLAUDE.md, and missed docs/60
   --strict bullets, the docs/20 NPC section note, STATUS.md L51, and cross-refs from the coyote_time_secs/ground_cast_length rows.
2. The "tread >= ~0.5 m" stair rule was a guess; the clear-path cast (radius+travel+margin from a pose already ~radius behind the riser)
   implies ~2x radius+, so real-world stairs (0.28 tread) won't climb. Asked for Task 1 to measure min tread.

Reusable facts:
- `player_p1_split_ring` is shared by local_coop_demo room9 AND room10. Any movement change on it hits both rooms.
- `blank_project` player prefab is the copy-from template. Recommend a commented line there for any new opt-in movement field.
- 3rd_person_game_demo has ONE player prefab (movement only sets coyote_time_secs).
- Orbit camera has no follow smoothing; Follow mode has `smoothing`. Any instant Y teleport pops the Orbit view.
- No documented invisible collider-only primitive exists in docs/20, so "use a ramp under stairs" isn't designer-authorable.

**How to apply:** when step_height ships, check that docs/20 publishes a MEASURED tread rule and the docs/60 bullets exist. It adds to
the coupled-field set in [[jump-rearm-coupling]] (step_height vs collider_radius + ground_cast_length reach for descent).
