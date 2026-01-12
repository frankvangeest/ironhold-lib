
# Ironhold-lib: Overview

Ironhold-lib is a cross-platform (native + web/WASM) game runtime built on Bevy.
Games are defined by data files (`assets/project.ron`, `assets/scenes/*.ron`) and assets
(models, audio, textures). Game creators can produce new projects/scenes without recompiling
the engine.

Key ideas:
- **Data-defined games**: project + scenes defined in RON.
- **Capability blocks**: engine provides reusable systems (controller, camera, UI, animation).
- **Cross-platform parity**: one core runtime, native and web entry points.
- **Future multiplayer**: architecture aims to support deterministic simulation and networking later.

See:
- Architecture: `docs/10_architecture.md`
- Data formats: `docs/20_data_formats.md`
- Runtime events + logic: `docs/30_runtime_events_and_logic.md`
- Determinism + networking: `docs/40_determinism_and_networking.md`
- Roadmap: `docs/50_roadmap_and_milestones.md`
``
