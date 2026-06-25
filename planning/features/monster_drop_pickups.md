# Feature: Monster Drop Pickups

_Status: Draft_
_Planned at: `7e9eb47` (2026-06-15)_

## What

When an enemy dies, it drops a pickup item — a health potion, coin stack, or gem — that appears
at the enemy's death position, bobs/spins in place, and can be collected by pressing F. Collecting
a health potion restores player HP; collecting coins/gems increments a `player_gold` stat. Each
drop type is a prefab with a behavior file so all pickup logic (stat change, sound, despawn, float
text) lives in RON with no engine recompile required.

## Why

- **Exercises the full dynamic-spawn pipeline.** The dynamic-spawn fix (motion + interactable +
  stat_label on `Action::Spawn`) now works correctly in engine code; this feature is the first
  real-game use of it, covering the entire path end-to-end in a playable project.
- **Meaningful gameplay loop.** Combat currently has no reward beyond the kill text. Drops give
  players an incentive to fight and something to do after an enemy falls.
- **Low implementation cost.** Most of the mechanic is already wired: `interactable` (F key),
  `motion` (bob/spin), `ModifyStat`, `Despawn`, `ShowFloatingText`, `PlaySound`. The only missing
  piece is spawning the drop AT the enemy's world position, which requires one small schema field.

## Approach

### 1 — New `at_entity` field on `Action::Spawn` (minimal Rust)

Add `at_entity: Option<String>` to `Action::Spawn` in `schema/actions.rs`:

```ron
// behavior file — drops a health potion where this entity currently is
Spawn(prefab: "drop_health_potion", id: "{self}_drop", at_entity: "{self}")
```

The executor resolves the named entity via `SpawnRegistry` → `GlobalTransform` and uses that as
the spawn position (with a small Y offset of `+0.3 m` so the drop sits just above ground).
If the entity is not found (already despawned), the spawn is silently skipped.
`at_entity` takes precedence over `position` and `spawn_point`.

All fields on `Action::Spawn` are `#[serde(default)]` so existing RON files are unaffected.

### 2 — New drop prefabs (RON only)

Three drop types, all GLB Prop kind with `interactable`, `motion`, and a `behavior`:

| Prefab key           | Model                  | Motion                          | Reward              |
|----------------------|------------------------|---------------------------------|---------------------|
| `drop_health_potion` | `health_pickup.glb`    | bob `(0.3, 2.0)`, no rotate     | `+25` player_health |
| `drop_coin`          | `stack_of_coins_01.glb`| bob `(0.15, 3.0)`, rotate Y 90° | `+1` player_gold    |
| `drop_gem`           | `gem.glb`              | bob `(0.2, 2.5)`, rotate Y 60°  | `+3` player_gold    |

All drops have `interactable: (radius: 2.0)`. No stat bar/label needed on drops — they're
single-use items, not tracked entities.

### 3 — Per-drop behavior files (`behaviors/drop_*.behavior.ron`)

Each drop handles its own `entity.interacted:{self}` event. Example for health potion:

```ron
// behaviors/drop_health.behavior.ron
(
    schema_version: 1,
    initial_state: "waiting",
    global_on: [],
    states: [
        (
            name: "waiting",
            entry_actions: [
                // Auto-despawn after 20 s if the player ignores it.
                EmitEventAfterDelay(event: "drop.expired:{self}", delay_secs: 20.0),
            ],
            exit_actions: [],
            on: [
                (
                    event: "entity.interacted:{self}",
                    do_actions: [
                        ModifyStat(key: "player_health", delta: 25.0),
                        PlaySound(key: "pickup_health"),
                        ShowFloatingText(entity: "player_01", text: "+25 HP"),
                        SpawnEffect(key: "pickup_sparkle", entity: "{self}"),
                        Despawn("{self}"),
                    ],
                ),
                (
                    event: "drop.expired:{self}",
                    do_actions: [ Despawn("{self}") ],
                ),
            ],
        ),
    ],
    transitions: [],
)
```

Coins and gems follow the same pattern — swap `ModifyStat(key: "player_gold", delta: 1.0)` and
`ShowFloatingText(entity: "player_01", text: "+1 Gold")`.

### 4 — Enemy behavior change (dead state)

In `enemy_snake.behavior.ron` and `enemy_spider.behavior.ron`, add to `dead` state entry_actions:

```ron
Spawn(prefab: "drop_health_potion", id: "{self}_drop", at_entity: "{self}"),
```

Orc gets a coin drop:

```ron
Spawn(prefab: "drop_coin", id: "{self}_drop", at_entity: "{self}"),
```

The drop ID `"{self}_drop"` is deterministic and unique per enemy instance. Since enemies respawn,
the old drop entity is already despawned (either collected or auto-expired) before the next death
generates a new `"{self}_drop"`.

### 5 — New stat: `player_gold` (stats.ron)

```ron
"player_gold": (
    base: 0.0,
    min: 0.0,
    max: 9999.0,
    regen_rate: 0.0,
    regen_delay: 0.0,
    thresholds: [],
),
```

Reset on each play session in `state_machine.ron` playing entry_actions:
`SetStat(key: "player_gold", value: 0.0)`.

### 6 — New audio catalog entries (assets.ron)

```ron
"pickup_health": ( path: "shared/audio/game-pickup-01.wav", volume: 0.9 ),
"pickup_coin":   ( path: "shared/audio/game-pickup-02.wav", volume: 0.7 ),
```

### 7 — New particle effect: `pickup_sparkle` (assets.ron)

Small bright burst fired when any item is collected — additive, short lifetime, upward velocity:

```ron
"pickup_sparkle": (
    particle_count: 12,
    lifetime_secs: 0.5,
    speed: 3.0,
    spread_deg: 60.0,
    size: 0.08,
    size_end: 0.0,
    color_start: (1.0, 1.0, 0.5, 1.0),
    color_end:   (1.0, 0.8, 0.1, 0.0),
    gravity: 2.0,
    additive: true,
),
```

### 8 — state_machine.ron playing entry_actions additions

```ron
PreloadPrefab("drop_health_potion"),
PreloadPrefab("drop_coin"),
PreloadPrefab("drop_gem"),
SetStat(key: "player_gold", value: 0.0),
SpawnEffect(key: "pickup_sparkle", position: (0.0, -100.0, 0.0)),  // pipeline warmup
```

## Tasks

- [ ] **Schema**: add `at_entity: Option<String>` to `Action::Spawn` in `schema/actions.rs`
- [ ] **Executor**: resolve `at_entity` in `action_executor.rs` — query `SpawnRegistry` + `GlobalTransform`; if entity not found, skip spawn (log warning); apply Y+0.3 offset
- [ ] **CLI**: run `cargo check -p ironhold_cli` — `query.rs` picks up new field automatically since it's `#[serde(default)]` and has no display logic
- [ ] **stats.ron**: add `player_gold`
- [ ] **assets.ron**: add `pickup_health`, `pickup_coin` audio keys; add `pickup_sparkle` effect; add model paths for `health_pickup`, `stack_of_coins_01`, `gem`
- [ ] **prefabs.ron**: add `drop_health_potion`, `drop_coin`, `drop_gem`
- [ ] **behavior files**: `behaviors/drop_health.behavior.ron`, `behaviors/drop_coin.behavior.ron`, `behaviors/drop_gem.behavior.ron`
- [ ] **Enemy behaviors**: add `Spawn(...at_entity...)` to dead state of enemy_snake, enemy_spider, enemy_orc
- [ ] **state_machine.ron**: add preloads, gold reset, sparkle warmup to playing entry_actions
- [ ] **Tests**: integration test for `at_entity` field — verify spawn position matches entity world transform; test that drop behavior file fires `Despawn` on interact
- [ ] **Docs**: `docs/20_data_formats.md` — document `at_entity` field on `Action::Spawn`; note auto-expire pattern in behavior examples
- [ ] **asset_manifest**: run `python tools/build_asset_manifest.py` after adding models to assets.ron

## Relationship to Other Features

**`at_entity` is a general engine primitive, not a loot-only concept.** This feature is the smallest vehicle that ships it end-to-end; but it is useful for any system that needs to spawn an entity at another entity's world position — spawn waves, scripted cutscenes, impact decals, loot bags. Do not scope it as loot infrastructure; it lives on `Action::Spawn` and is available everywhere `Spawn` is.

**Loot System v1** (`planning/features/loot_system.md`) has a soft dependency on this feature. Its `RollLootTable` executor currently resolves `GlobalTransform` by hand to position the loot bag. Once `at_entity` lands, the loot executor should drop that hand-rolled lookup and use `at_entity: Some(entity.clone())` — closing a duplicate code path. If monster drops ships first (it should; it has no inventory dep), the loot executor is written against `at_entity` from the start. If loot ships first, the manual transform lookup must be replaced when this feature lands.

The `drop_table: Option<String>` extension noted below is the on-ramp toward loot tables — it would use `RollLootTable` under the hood once the loot system exists.

## Open questions

- **Multiple drops per enemy?** Not in scope for v1. A future extension could be a `drop_table` on `PrefabDef` (probabilities + drop pool) — noted for later.
- **Gold UI display?** A HUD label bound to `player_gold` stat via `StatValueText` would complete the loop, but the main.scene.ron UI already has a stat bar row. Decide during implementation — if simple, wire it; if not, defer.
- **Drop collision with physics?** Drops are Prop kind; they don't have a physics body. They float at spawn Y. If terrain is uneven an enemy dying on a slope may leave the drop floating in air — acceptable for v1.
- **Sound for gem pickup?** We only have two pickup WAVs. Use `pickup_coin` sound for gem drops in v1; can add a third audio file later.

## Acceptance criteria

- Given an enemy_snake or enemy_spider dies, a health_pickup GLB appears at its death position, bobbing up and down.
- Given the player walks within 2 m of the drop and presses F, the item despawns, player health increases by 25, "+25 HP" floats above the player, and a sparkle particle fires.
- Given the player ignores a drop for 20 seconds, it despawns automatically.
- Given an enemy_orc dies, a coin stack appears and collecting it increments `player_gold` by 1.
- All existing prefab/entity behavior in `3rd_person_game_demo` is unaffected (no regression in health bars, combat, respawn cycle).
- `cargo check -p ironhold_cli` passes after schema change.
