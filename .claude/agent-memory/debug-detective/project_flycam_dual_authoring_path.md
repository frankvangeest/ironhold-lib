---
name: flycam-dual-authoring-path
description: A flycam's FlyCamDef reaches the runtime via TWO authoring paths (legacy `flycam:` and `camera_mode: Flycam(...)`); any validator gated on one is blind to the other, and 2 shipped projects use the legacy one
metadata:
  type: project
---

`scene_loader.rs`'s flycam branch resolves the effective mode as
`components.camera_mode.unwrap_or_else(|| Flycam(components.flycam.unwrap_or_default()))` — so an
identical `FlyCamDef` (movement keys, `look_button`) reaches the runtime through either
`camera_mode: Flycam(...)` **or** the legacy bare `flycam:` field. `foliage_demo` and
`dynamic_animation_control` are the shipped projects on the legacy path; `camera_modes` is on the
modern one.

**Why:** a CLI validate check gated on `components.camera_mode` being `Some` (as
`camera_mode_vocab_problems`'s call sites were, batch3 2026-09-11) silently passes a project whose
flycam authoring is entirely in `flycam:` — including the movement-key typo that has *no runtime
warn at all*. Same class as [[project_logic_file_on_disk_is_not_loaded]]: the field a validator
reads is not the field the runtime reads.

**How to apply:** any new flycam/camera check must cover both `def.components.camera_mode` and
`def.components.flycam`. When probing, `tags: ["flycam"]` + `flycam: (forward: "Foward")` and no
`camera_mode` is the one-file fixture that separates the two paths.

Related trap in the same area: the six flycam movement keys do **not** all fall back to `KeyW` —
`InputMap::parse_key(..).unwrap_or(...)` uses KeyW/KeyS/KeyA/KeyD/**Space**/**KeyQ** per field, at
all three spawn sites. Any diagnostic naming a single fallback key is wrong for 5 of 6 fields.
