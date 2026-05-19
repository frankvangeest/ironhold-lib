# RON Project Authoring Guide

Rules for creating and editing projects under `assets/projects/`. Read this before writing any `.ron` files.

---

## Schema versions

Every RON file must start with `schema_version`. Use these values:

| File | schema_version |
|---|---|
| `*.project.ron` | 3 |
| `*.scene.ron` | 2 |
| `assets.ron` | 1 |
| `prefabs/prefabs.ron` | 1 |
| `logic/rules.ron` | 2 |
| `logic/state_machine.ron` | 1 |
| `behaviors/*.behavior.ron` | 1 |
| `stats/stats.ron` | 1 |

---

## Action RON syntax — struct vs tuple variants

**This is the most common source of parse errors.** Whether an action uses named fields or positional args depends on how it is declared in `schema/actions.rs`.

### Tuple variants → positional (no field names)

```ron
LoadScene("scenes/game.scene.ron")
Despawn("enemy_01")
EnterState("playing")
EmitEvent("player.died")
SetVariable("score", "0")       // TWO positional strings
IncrementVariable("score", 1)   // string key, then i32 delta
SetVolume(80)
PreloadScene("scenes/game.scene.ron")
PreloadPrefab("enemy_orc")
Log("debug message")
Quit
StopMusic
UnloadOverlay
ToggleOverlay("scenes/pause.scene.ron")
LoadSceneOverlay("scenes/pause.scene.ron")
PlayAnimation("run")
```

### Struct variants → named fields

```ron
Spawn(prefab: "enemy_orc", id: "orc_01", position: (5.0, 0.5, 0.0))
PlaySound(key: "pickup_coin", volume: 0.8)
PlayMusicLoop(key: "bg_forest")
PlayAnimationOn(target: "{self}", clip: "attack")
EmitEventAfterDelay(event: "enemy.respawn:{self}", delay_secs: 5.0)
SpawnEffect(key: "hit_spark", entity: "{self}")
SpawnEffect(key: "explosion_burst", position: Some((0.0, 0.5, 0.0)))
ModifyStat(key: "health", delta: -25.0)
SetStat(key: "health", value: 100.0)
ApplyModifier(modifier_key: "speed_boost")
RemoveModifier(modifier_key: "poison")
ShowDamagePopup(entity: "{self}", amount: -25.0)
SetEntityVisible(entity: "{self}", visible: false)
```

**Rule of thumb**: if it looks like `Foo(String)` or `Foo(String, i32)` in `schema/actions.rs`, use positional. If it has named fields (`Foo { key: String, ... }`), use named fields. When in doubt, check `crates/ironhold_core/src/schema/actions.rs`.

---

## `{self}` substitution in behavior files

Inside `*.behavior.ron`, `{self}` in any event name or action string is replaced at runtime with the entity's spawn ID. This makes behavior files reusable across multiple instances:

```ron
// Works for every entity that uses this behavior file:
SpawnEffect(key: "hit_spark", entity: "{self}")
EmitEventAfterDelay(event: "enemy.died:{self}", delay_secs: 0.2)
Despawn("{self}")
ModifyStat(key: "{self}.health", delta: -10.0)
```

---

## Rules file structure

```ron
(
    schema_version: 2,
    rules: [
        ( on: "scene.ready:main",            do_actions: [ ... ] ),
        ( on: "ui.button_pressed:btn_start", do_actions: [ ... ] ),
        ( on: "entity.entered:pad_01",       do_actions: [ ... ] ),
        ( on: "entity.interacted:rune_01",   do_actions: [ ... ] ),
    ],
)
```

Common event name patterns:
| Source | Event pattern |
|---|---|
| Scene loaded | `scene.ready:{scene_name}` |
| UI button | `ui.button_pressed:{button_action_field}` |
| TriggerZone enter | `entity.entered:{entity_id}` |
| TriggerZone exit | `entity.exited:{entity_id}` |
| Interactable [F] | `entity.interacted:{entity_id}` |
| EmitEvent / EmitEventAfterDelay | whatever string was passed |

---

## Behavior file structure

```ron
(
    schema_version: 1,
    initial_state: "idle",
    global_on: [],
    states: [
        (
            name: "idle",
            entry_actions: [ EmitEventAfterDelay(event: "loop.tick:{self}", delay_secs: 0.5) ],
            exit_actions: [],
            on: [
                (
                    event: "loop.tick:{self}",
                    do_actions: [
                        SpawnEffect(key: "sparkle", entity: "{self}"),
                        EmitEventAfterDelay(event: "loop.tick:{self}", delay_secs: 0.5),
                    ],
                ),
            ],
        ),
    ],
    transitions: [],
)
```

---

## Effect definition fields (in `assets.ron`)

```ron
"my_effect": (
    particle_count: 20,           // required; max 256
    lifetime_secs: 1.2,           // required
    speed: 3.0,                   // default 0.0
    speed_jitter: 0.5,            // default 0.0
    spread_deg: 90.0,             // default 180.0 (full sphere)
    emit_radius: 0.3,             // default 0.0 (point)
    offset: (0.0, 1.0, 0.0),      // default (0, 1, 0)
    size: 0.2,                    // default ~0.06
    size_end: 0.0,                // default None (constant)
    size_jitter: 0.05,            // default 0.0
    color_start: (1.0, 0.5, 0.1, 1.0),
    color_mid:   (1.0, 0.2, 0.0, 0.8),  // optional 3-stop gradient
    color_end:   (0.3, 0.0, 0.0, 0.0),
    gravity: -4.0,                // default 0.0; negative = falls
    turbulence: 0.5,              // default 0.0
    sprite: "particle/flame_04",  // single texture key from assets.ron
    // OR:
    sprites: ["particle/flame_01", "particle/flame_02"],  // random pick per particle
    additive: true,               // default false (Blend); true = Add (fire/glow)
    uv_distort: 0.4,              // default 0.0; flame shader only
    uv_scroll_speed: 0.5,         // default 0.0; flame shader only
),
```

Using both `uv_distort`/`uv_scroll_speed` and a sprite key routes the particle through `PoolFlameMaterial` (animated flame). Without those fields, sprites use `StandardMaterial`.

---

## WebGPU pipeline warmup

On first load, WASM lazily compiles a GPU pipeline for each material+blend variant. Without warmup this causes a visible frame stall when the first burst fires. Warm all variants you use by firing them off-screen on `scene.ready`:

```ron
( on: "scene.ready:main", do_actions: [
    SpawnEffect(key: "explosion_burst", position: Some((0.0, -100.0, 0.0))),  // additive sphere
    SpawnEffect(key: "star_rain",       position: Some((0.0, -100.0, 0.0))),  // additive sprite
    SpawnEffect(key: "campfire_smoke",  position: Some((0.0, -100.0, 0.0))),  // blend sprite
    SpawnEffect(key: "campfire_body",   position: Some((0.0, -100.0, 0.0))),  // flame material
]),
```

---

## New project checklist

1. Create `assets/projects/{name}/` with: `{name}.project.ron`, `scenes/main.scene.ron`, `assets.ron`, `prefabs/prefabs.ron`, `logic/rules.ron`
2. Register in `test_web.py` — append name to `PROJECTS` list
3. Add a card to `index.html` — copy an existing `<a class="project-card">` block
4. Generate baseline screenshot: `python test_web.py --project {name} --update-baselines --skip-build`
5. Run asset checker: `python tools/asset_checker/check.py`
