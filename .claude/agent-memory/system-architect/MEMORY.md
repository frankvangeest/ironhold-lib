# Memory Index — system-architect

- [Core architectural decisions](arch_decisions.md) — three-crate split, Message→Action pipeline, ActionQueue FIFO, asset catalog pattern, schema as designer API
- [Fragile modules](fragile_modules.md) — composite prefab spawning, EffectDef/LayerDef sync, WebGPU alignment, particle warmup, terrain async, spawn queue cap
- [Capability patterns](capability_patterns.md) — how to add capabilities/actions/events, rules.ron vs state_machine.ron, schema stability rules, physics/inspector constraints
- [WASM pitfalls](wasm_pitfalls.md) — pipeline compilation latency, no threading, 16-byte alignment, binary size limit, asset preloading
- [Scene/prefab boundary](scene_prefab_boundary.md) — how scene vs prefab responsibilities diverge from engine canon; no per-instance overrides; recommended direction
- [GPU physics / wgrapier](gpu_physics_wgrapier.md) — wgrapier is WIP/throwaway prototype, no Bevy bridge; CPU height-array is correct for terrain ground queries; defer GPU physics
- [Player spawn paths](player_spawn_paths.md) — three player-construction sites; shared-helper rule; executor-side tag detection; dual-camera/tonemapping/terrain-timing caveats for runtime player spawn
