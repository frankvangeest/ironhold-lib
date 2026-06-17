# Feature: Creature Collider Sizing — Snake & Spider

_Status: Ready_
_Planned at: `0f86e07` (2026-06-17)_

## What

Tune the physics capsule dimensions on the `enemy_snake` and `enemy_spider` prefabs so they match the visual footprint of their respective GLB models. Both currently inherit the default humanoid capsule (0.35 m radius, 1.6 m height), which overshoots both creatures significantly.

## Why

The default humanoid capsule is visually wrong for low-profile creatures. The snake's 1.6 m capsule stands nearly twice the model's height; the spider's capsule is similarly tall for a multi-legged low-to-ground model. An oversized capsule blocks player approach at `approach_distance: 1.5 m` and makes melee hits feel off — the player stops moving before they visually reach the creature.

## Approach

Pure RON data change — no Rust code required. Edit `collider_height` and `collider_radius` in the `npc:` block of each prefab in `assets/projects/3rd_person_game_demo/prefabs/prefabs.ron`. Starting values from in-engine observation:

| Prefab | `collider_height` | `collider_radius` |
|---|---|---|
| `enemy_snake` | 0.8 | 0.3 |
| `enemy_spider` | 1.2 | 0.4 |

Verify by play-testing in `3rd_person_game_demo` — spawn each creature, walk into it, and confirm the capsule boundary matches the body. Adjust from the starting values if needed.

## Tasks

- [ ] Set `collider_height: 0.8, collider_radius: 0.3` on `enemy_snake` in `prefabs.ron`
- [ ] Set `collider_height: 1.2, collider_radius: 0.4` on `enemy_spider` in `prefabs.ron`
- [ ] Play-test in `3rd_person_game_demo` — confirm capsule boundary matches each body, melee approach feels natural
- [ ] Adjust values if observation disagrees with starting estimates

## Open questions

- Should the snake collider be a sphere rather than a capsule given its flat, elongated body? (The engine currently only supports `CapsuleY`; defer to a future collider-shape feature if this matters.)

## Acceptance criteria

- Standing next to `enemy_snake` no longer feels like bumping into invisible air above the model.
- `approach_distance: 1.5` on NPC brings the player visually alongside the snake, not hovering 1–2 m away.
- Same for `enemy_spider` — no oversized invisible barrier around a floor-level body.
