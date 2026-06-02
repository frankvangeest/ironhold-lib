# Feature: Day/Night Cycle

_Status: Draft_
_Planned at: `4c47cc6` (2026-06-02)_

---

> ## Pre-implementation checklist
>
> - [ ] **Decide: keyframe definition style.** Options: (a) named keyframes at fixed hours (`dawn`, `noon`, `dusk`, `midnight`) with a required 4-entry list; (b) arbitrary `(hour, ...)` keyframes — any count, any hour value, sorted by hour. Recommendation: **arbitrary keyframes** — more flexible for unusual cycles (always-night dungeon, two-sun world); the designer specifies how many they need. Minimum 2 keyframes required for interpolation; the cycle wraps (midnight interpolates back to dawn).
>
> - [ ] **Decide: time-of-day events — fixed names or keyframe-defined.** The backlog mentions `time.dawn`, `time.noon`, etc. Options: (a) hardcoded event names triggered at 6h/12h/18h/0h; (b) each keyframe in the RON has an optional `event: String` field; the system emits it once per crossing. Recommendation: **optional `event` on each keyframe** — decouples event names from hour numbers, works for non-Earth cycles (48-hour day, etc.), and is fully designer-controlled.
>
> - [ ] **Decide: multiple directional lights.** `SceneLightingV2` supports multiple `point_lights` but only one `directional` light. The day/night cycle drives that one directional light. If a scene has no directional light in its static lighting, the system spawns one. Confirm: is it acceptable to always spawn a directional light when `day_night_cycle` is defined, even if `lighting.directional` is `None` in the scene? Recommendation: **yes** — a scene with a day/night cycle implicitly needs a sun; if no directional is declared, spawn a default one.
>
> - [ ] **Decide: `TimeOfDay` units.** Hours (0.0–24.0) are human-readable. Normalized (0.0–1.0) is easier for math. Recommendation: **hours 0.0–24.0** in the schema for authoring clarity; the system normalizes internally when needed. `SetTimeOfDay(12.0)` means noon.
>
> - [ ] **Decide: sky color / fog.** Does the day/night cycle drive sky color? `SceneLightingV2` has no sky box or procedural sky. Fog (`FogSettings` in Bevy) is not in the current schema. For v1, **only drive directional light + ambient light**. Sky and fog can be added in a follow-up pass.

---

## What

A scene-level system that animates the sun (directional light) and ambient light through a 24-hour cycle, interpolating designer-authored keyframes. Fully WASM-compatible (pure CPU math, no post-processing, no HDR required).

Designers declare `day_night_cycle` in scene RON with a `cycle_duration_secs` and a list of keyframes, each specifying sun color, sun intensity, ambient color, ambient brightness, and an optional pipeline event to emit when that hour is crossed.

Two new actions — `SetTimeOfDay` and `SetDaySpeed` — let designers trigger sunrise cutscenes, jump to midnight on a trigger, or slow/pause time in a boss encounter.

---

## Why

Static lighting limits world believability and event scripting options. A data-driven day/night cycle:
- Enables timed gameplay (defend at night, gather resources at day).
- Costs nothing on WASM — pure lerp between `Color` and `f32` values each frame.
- Does not require HDR, post-process, or multi-pass rendering.

---

## Schema

### `GameSceneV2` — new field (`schema/scene_v2.rs`)

```ron
// scenes/open_world.scene.ron
(
    lighting: (
        ambient: (0.4, 0.45, 0.5),
        ambient_brightness: 120.0,
        directional: (
            color: (1.0, 0.95, 0.8),
            illuminance: 10000.0,
            direction: (-0.5, -1.0, -0.3),
        ),
        // day_night_cycle overrides the static directional/ambient above each frame
        day_night_cycle: Some((
            cycle_duration_secs: 600.0,  // 10-minute in-game day
            start_hour: 6.0,             // begin at dawn on scene load
            speed_multiplier: 1.0,       // can be overridden by SetDaySpeed action
            keyframes: [
                ( hour: 0.0,  sun_color: (0.05, 0.05, 0.15), sun_illuminance: 200.0,
                  ambient: (0.05, 0.05, 0.1), ambient_brightness: 40.0,
                  event: Some("time.midnight") ),
                ( hour: 6.0,  sun_color: (1.0, 0.6, 0.3), sun_illuminance: 4000.0,
                  ambient: (0.4, 0.35, 0.3), ambient_brightness: 80.0,
                  event: Some("time.dawn") ),
                ( hour: 12.0, sun_color: (1.0, 0.98, 0.9), sun_illuminance: 12000.0,
                  ambient: (0.5, 0.5, 0.5), ambient_brightness: 180.0,
                  event: Some("time.noon") ),
                ( hour: 18.0, sun_color: (1.0, 0.5, 0.2), sun_illuminance: 3000.0,
                  ambient: (0.35, 0.25, 0.2), ambient_brightness: 60.0,
                  event: Some("time.dusk") ),
            ],
        )),
    ),
    // ...
)
```

```rust
// schema/scene_v2.rs — in SceneLightingV2
#[serde(default)]
pub day_night_cycle: Option<DayNightCycleDef>,
```

### New `DayNightCycleDef` + `DayKeyframeDef` (`schema/scene_v2.rs`)

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DayNightCycleDef {
    /// Real-world seconds for one full 24-hour in-game day. Default: 300.0 (5 minutes).
    #[serde(default = "default_cycle_duration")]
    pub cycle_duration_secs: f32,

    /// In-game hour (0.0–24.0) when the scene starts. Default: 6.0 (dawn).
    #[serde(default = "default_start_hour")]
    pub start_hour: f32,

    /// Speed multiplier applied on top of the cycle rate. Default: 1.0.
    /// 0.0 freezes time; 2.0 runs twice as fast. Overridable via SetDaySpeed action.
    #[serde(default = "default_speed_multiplier")]
    pub speed_multiplier: f32,

    /// At least 2 keyframes required. Sorted by `hour` ascending at load time.
    /// The cycle wraps: after hour 24 it interpolates back to hour 0.
    pub keyframes: Vec<DayKeyframeDef>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DayKeyframeDef {
    /// In-game hour this keyframe represents. Range: 0.0–24.0.
    pub hour: f32,

    /// Directional light (sun) color as linear RGB 0.0–1.0.
    pub sun_color: (f32, f32, f32),

    /// Directional light illuminance in lux. Typical range: 200 (night) – 15000 (noon).
    pub sun_illuminance: f32,

    /// Ambient light color as linear RGB 0.0–1.0.
    pub ambient: (f32, f32, f32),

    /// Ambient light brightness (lux). Typical range: 30–200.
    pub ambient_brightness: f32,

    /// Optional event to emit once when this keyframe hour is crossed (rising edge only).
    #[serde(default)]
    pub event: Option<String>,
}
```

### New actions (`schema/actions.rs`)

```ron
SetTimeOfDay(12.0)       // jump to noon immediately (no interpolation)
SetDaySpeed(0.0)         // freeze time
SetDaySpeed(1.0)         // restore normal speed
SetDaySpeed(10.0)        // 10× speed (time-lapse)
```

```rust
/// Instantly set the in-game hour (0.0–24.0). Clamps to valid range.
/// Updates `TimeOfDay` resource; keyframe events do NOT fire for skipped hours.
SetTimeOfDay(f32),
/// Set the day/night cycle speed multiplier. 0.0 = frozen; 1.0 = normal; >1.0 = fast.
/// Persists until changed again or scene transition resets it to the scene default.
SetDaySpeed(f32),
```

---

## Runtime

### Resources (`capabilities/day_night.rs`)

```rust
/// Current in-game time. Mutated each frame by `day_night_tick_system`.
#[derive(Resource, Default)]
pub struct TimeOfDay(pub f32);  // hours, 0.0–24.0

/// Loaded from scene RON on scene load. None when scene has no cycle.
#[derive(Resource, Default)]
pub struct DayNightConfig(pub Option<DayNightCycleDef>);

/// Tracks which keyframe hours have already fired their event this cycle.
/// Reset on scene load and on `SetTimeOfDay` (skip-ahead suppresses events for jumped hours).
#[derive(Resource, Default)]
pub struct DayNightEventState {
    pub fired_this_cycle: HashSet<String>,  // keyed by event string
    pub current_speed: f32,                 // overridden by SetDaySpeed
}
```

### `day_night_tick_system` (`capabilities/day_night.rs`)

Runs in `Update`. Only active when `DayNightConfig` is `Some`.

1. **Advance time**: `time_of_day.0 += delta_secs * (24.0 / cycle_duration_secs) * speed_multiplier`. Wrap at 24.0.
2. **Find bracket**: binary search the sorted keyframes for the two surrounding the current hour. Handle wrap-around (midnight → dawn).
3. **Lerp**: compute `t = (current_hour - prev.hour) / (next.hour - prev.hour)`. Lerp sun color, sun illuminance, ambient color, ambient brightness.
4. **Apply**: update `DirectionalLight` color + illuminance and `AmbientLight` color + brightness via ECS queries. Use **change-detection guard** — only write if value changed by > 0.5% to avoid driving render every frame.
5. **Event crossing**: for each keyframe with `event: Some(...)`, check if the current tick crossed that hour (previous tick hour < keyframe hour ≤ current tick hour, handling wrap). On crossing: if not already in `fired_this_cycle`, emit the event via `GameEvent::Trigger` and add to `fired_this_cycle`. Clear `fired_this_cycle` on full cycle wrap.

### `SetTimeOfDay` executor arm

```rust
Action::SetTimeOfDay(hour) => {
    time_of_day.0 = hour.clamp(0.0, 24.0);
    day_night_events.fired_this_cycle.clear();  // suppress events for skipped hours
}
Action::SetDaySpeed(multiplier) => {
    day_night_events.current_speed = multiplier.max(0.0);
}
```

### Direction of the sun

The scene's static `lighting.directional.direction` is used as the **base** sun direction at noon. The day/night cycle rotates the directional light around the east-west axis proportional to hour. At midnight the light points straight down (from below, effectively off); at noon it points from above.

```rust
// Hour 0 = midnight (from below), 6 = sunrise (east), 12 = noon (overhead), 18 = sunset (west)
let angle = (time_of_day.0 / 24.0) * std::f32::consts::TAU;
let dir = Vec3::new(angle.sin(), -angle.cos(), -0.5).normalize();
// Only update directional light transform when angle changes by > 0.1°
```

This is the default. If a scene's `lighting.directional` is authored with a fixed direction, that direction is used as the noon direction and the system rotates around it.

---

## Designer usage patterns

**Freeze time at dusk for a boss encounter:**
```ron
( on: "boss.phase2", do_actions: [ SetTimeOfDay(17.5), SetDaySpeed(0.0) ] ),
( on: "boss.defeated", do_actions: [ SetDaySpeed(1.0) ] ),
```

**Always-night dungeon:**
```ron
day_night_cycle: Some((
    cycle_duration_secs: 99999.0,  // effectively frozen
    start_hour: 0.0,
    speed_multiplier: 0.0,
    keyframes: [
        ( hour: 0.0, sun_color: (0.05, 0.05, 0.15), sun_illuminance: 200.0,
          ambient: (0.05, 0.05, 0.1), ambient_brightness: 40.0, event: None ),
        ( hour: 12.0, sun_color: (0.05, 0.05, 0.15), sun_illuminance: 200.0,
          ambient: (0.05, 0.05, 0.1), ambient_brightness: 40.0, event: None ),
    ],
)),
```

**React to time of day for gameplay:**
```ron
( on: "time.dusk", do_actions: [ EmitEvent("enemies.emerge"), PlaySound(key: "night_ambience") ] ),
( on: "time.dawn", do_actions: [ EmitEvent("enemies.retreat"), PlaySound(key: "birds") ] ),
```

---

## New Rust changes

- `schema/scene_v2.rs` — `DayNightCycleDef`, `DayKeyframeDef` structs; `day_night_cycle: Option<DayNightCycleDef>` on `SceneLightingV2`.
- `schema/actions.rs` — `SetTimeOfDay(f32)`, `SetDaySpeed(f32)`.
- `capabilities/day_night.rs` (new file) — `TimeOfDay`, `DayNightConfig`, `DayNightEventState`, `day_night_tick_system`.
- `capabilities/mod.rs` — register module + system.
- `runtime/scene_manager/action_executor.rs` — handle `SetTimeOfDay`, `SetDaySpeed`.
- `runtime/scene_manager/scene_loader.rs` — populate `DayNightConfig` + `TimeOfDay` from scene lighting block on load; reset on `LoadScene`.

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] `DayNightCycleDef` + `DayKeyframeDef` in `schema/scene_v2.rs`
- [ ] `day_night_cycle` field on `SceneLightingV2`
- [ ] `SetTimeOfDay(f32)` + `SetDaySpeed(f32)` actions
- [ ] `TimeOfDay`, `DayNightConfig`, `DayNightEventState` resources
- [ ] `day_night_tick_system` — advance, lerp, apply, event crossing
- [ ] Sun direction rotation logic (east-west axis)
- [ ] Change-detection guard on light component writes
- [ ] Scene loader populates / resets day-night resources
- [ ] Demo: add `day_night_cycle` to `terrain_demo` or `3rd_person_game_demo`; wire `time.dawn`/`time.dusk` events to ambient sound changes
- [ ] Integration tests: time advances correctly, lerp between keyframes, event fires exactly once per crossing, `SetTimeOfDay` jump suppresses crossed events, `SetDaySpeed(0)` freezes
- [ ] Docs: `DayNightCycleDef` fields in `docs/20_data_formats.md`; `SetTimeOfDay`, `SetDaySpeed`, `time.*` events in `docs/30_runtime_events_and_logic.md`

---

## Open questions

- **Directional light direction when no static light defined**: if `lighting.directional` is `None` and `day_night_cycle` is set, spawn a default directional light. Need to track whether the scene has a "cycle-managed" directional so the loader can find and update it.
- **Multiple directional lights (moon + sun)**: out of scope for v1. A two-directional-light setup would need the cycle to manage both separately. Deferred.
- **WASM frame rate**: delta-based advancement is framerate-independent. No fixed-tick requirement for the cycle itself (light color changes are visually smooth even at variable rate).
- **`TimeOfDay` as a RON-readable variable**: designers may want to bind the current hour to a UI label. Expose `TimeOfDay.0` through `GameVariables` (or a dedicated `time_of_day` variable that the system writes each frame) — deferred to a follow-up.

---

## Acceptance criteria

- Given `day_night_cycle` in scene RON, the directional light color and intensity animate over time without any rule wiring.
- Given `keyframes` with `event: Some("time.dawn")`, that event fires once per crossing of the dawn hour.
- Given time wraps past 24.0, the cycle restarts and events fire again on the next crossing.
- Given `SetTimeOfDay(12.0)`, `TimeOfDay` jumps to 12.0 instantly; events for hours 0–11 do NOT fire.
- Given `SetDaySpeed(0.0)`, `TimeOfDay` stops advancing.
- Given `SetDaySpeed(2.0)`, the cycle runs at double speed.
- Given no `day_night_cycle` block in scene RON, no tick system runs and static lighting is unchanged.
- Given a scene transition (`LoadScene`), `TimeOfDay` resets to `start_hour` of the new scene.
