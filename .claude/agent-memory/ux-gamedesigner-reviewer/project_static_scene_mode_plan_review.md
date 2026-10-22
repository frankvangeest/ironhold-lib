---
name: static-scene-mode-plan-review
description: static_scene_mode (?static=1 harness freeze) plan-review 2026-10-23 - game.paused must track Menu hold only; scope to single-scene screenshots; motion projects need live captures
metadata:
  type: project
---

Plan-review at `e2a5578` (2026-10-23): verdict Needs more design work (small). Harness-only feature, so it stays out of designer docs.

Main findings:
- The plan offers two `game.paused` options ("emit only on Menu toggle" vs "skip while Static is the only hold"), and they behave differently when Static is already holding: with the second option a PauseGame changes nothing on the clock, so `game.paused` never fires and a pause menu wired to `game.paused` (first_person_starter, docs/30 recommends this) never opens. A `scene.ready` PauseGame also becomes order-dependent. Recommendation: events track the Menu hold only.
- `open_project()` is shared by pause_nav, which crosses 3 Replace loads. The Static hold persists across loads, so that flow would freeze at start_menu's Ready. Recommendation: an explicit `static=` parameter, used only by the single-scene screenshot path.
- Freezing at Ready bakes in a wrong picture: bodies frozen mid-fall, the camera follow/transition not settled, and `awaiting_reveal` models possibly still hidden. Particle warmup emitters freeze with 0 particles.
- Motion projects need live captures or an opt-out set (like NON_DETERMINISTIC_SCENES): particles_demo, effect_mayhem_demo, custom_materials, foliage_demo, dynamic_animation_control. Whether shader time freezes is still unverified.
- Acceptance "NPCs at spawn positions" contradicts "no NPC reset". "Pixel-identical" does not fit the 4% threshold. The pause / game.paused, pause_nav and native cases have no criteria.

**How to apply:** if the implementation comes back, check these points first. Related: [[real-pause-plan-review]], [[particle-warmup-recipe]], [[dynamic-animation-control-demo]].
