# Feature: Particle System v2 — 9. Shared Effect Library

_Status: Draft_
_Planned at: `2cc61ca` (2026-05-19)_
_Part of: see `planning/features/particle_system_v2.md` for the full v2 overview_

## What

A curated set of reusable particle effect definitions in `assets/shared/effects/`, each
a polished, self-contained RON file. Projects reference them by key and optionally
override specific top-level fields without duplicating the full definition.

## Why

Currently every project re-authors common effects (fire, smoke, healing burst, explosion)
from scratch. A new project's `assets.ron` must define all effects it uses even if they
are identical to another project's. Shared effects cut authoring time for new projects
to near zero for standard visuals, and give all projects a consistent quality baseline
that improves centrally.

## Approach

**Directory structure:**
```
assets/shared/effects/
  fire.ron        — campfire, torch, bonfire, ...
  magic.ron       — arcane nova, heal pulse, orbit ring, ...
  impact.ron      — explosion, frost burst, shockwave, ...
  ambient.ron     — smoke column, star shower, sparkles, ...
```

Each file is a RON map of `String → EffectDef` (same format as `AssetCatalog.effects`).

**Loading:** a `SharedEffectCatalog` resource is loaded at engine startup (before the
first scene), merged into `LoadedAssetCatalog` with a `shared/` namespace prefix.
Projects can then use `SpawnEffect(key: "shared/fire/campfire")` directly, or alias
the key in their `assets.ron`.

**Per-project overrides:** projects can reference a shared effect with field overrides:

```ron
// In project assets.ron
effects: {
  "boss_explosion": SharedEffect(
    base: "shared/impact/explosion",
    overrides: (
      light: ( color: (0.8, 0.2, 1.0), intensity: 15000.0, range: 10.0, fade_out_secs: 0.8 ),
    ),
  ),
},
```

Override depth: **shallow** — the overrides struct replaces whole top-level fields
(e.g. the entire `light` block), not individual sub-fields. This avoids surprising
partial-merge semantics.

**Initial shared effects** (promoted from particles_demo and polished):

| Key | Description |
|---|---|
| `shared/fire/campfire` | Multi-layer stationary campfire with dynamic light |
| `shared/fire/torch` | Small upward flame for wall torches |
| `shared/fire/explosion` | Sphere burst + flash light |
| `shared/magic/arcane_nova` | Orbiting ring burst |
| `shared/magic/heal_pulse` | Rising star bloom |
| `shared/magic/channel_ring` | Continuous orbiting rune loop |
| `shared/magic/frost_burst` | Ice shard hemisphere |
| `shared/ambient/smoke_column` | Rising grey smoke |
| `shared/ambient/star_shower` | Wide-scatter falling stars |
| `shared/ambient/sparkles` | Gentle ambient sparkle loop |

**New schema element:**

```rust
// In schema/catalog.rs
pub enum EffectEntry {
    Inline(EffectDef),                          // existing — full inline definition
    Shared { base: String, overrides: EffectDefOverrides }, // new — reference + overrides
}
```

`AssetCatalog.effects` changes from `HashMap<String, EffectDef>` to
`HashMap<String, EffectEntry>`. Resolved at catalog load time; the executor always works
with resolved `EffectDef` values.

**Asset checker extension:** validate that `base` keys in `SharedEffect` entries resolve
against the loaded shared catalog.

**Files:**
- `assets/shared/effects/fire.ron` (new)
- `assets/shared/effects/magic.ron` (new)
- `assets/shared/effects/impact.ron` (new)
- `assets/shared/effects/ambient.ron` (new)
- `schema/catalog.rs` — `EffectEntry` enum, `EffectDefOverrides`, `SharedEffectCatalog`
- `runtime/scene_manager/mod.rs` — load shared catalog at startup
- `tools/asset_checker/check.py` — validate shared effect base keys

## Tasks

- [ ] Design `SharedEffectCatalog` loading (loaded once, before first scene)
- [ ] Add `EffectEntry` enum to `schema/catalog.rs`; update `AssetCatalog.effects` type
- [ ] Implement `EffectDefOverrides` struct (subset of `EffectDef` top-level fields, all optional)
- [ ] Implement override merge (shallow replace of present fields)
- [ ] Load shared catalog in `runtime/scene_manager/mod.rs` at engine startup
- [ ] Write `fire.ron` (campfire, torch, explosion promoted from particles_demo)
- [ ] Write `magic.ron` (arcane_nova, heal_pulse, frost_burst, channel_ring)
- [ ] Write `impact.ron` (explosion, shockwave)
- [ ] Write `ambient.ron` (smoke_column, star_shower, sparkles)
- [ ] Update particles_demo to reference 3+ shared effects
- [ ] Update asset checker to validate `SharedEffect.base` keys
- [ ] Tests:
  - [ ] `SharedEffect(base: "shared/fire/campfire")` resolves to expected EffectDef
  - [ ] Override replaces `light` block correctly
  - [ ] Unknown `base` key fails validation
- [ ] Update `docs/20_data_formats.md`

## Open questions

- **Auto-namespace or explicit prefix**: should shared effects be referenced as
  `"shared/fire/campfire"` (explicit, clear) or `"fire/campfire"` (implicit, shorter)?
  Explicit is less surprising; use `shared/` prefix.
- **Override depth**: shallow (whole-field replace) is chosen for simplicity. If per-field
  override is needed later (e.g. just change `light.color`), add a `v2.1` deep-merge
  variant.
- **Versioning**: if a shared effect is updated, all projects referencing it get the new
  version on next build. This is the data-driven ideal. If a project needs to pin a
  specific version, copy the definition inline. No version pinning in the schema.
- **Shared prefab alignment**: shared effects reference shared texture keys. Ensure
  shared texture keys used in shared effects are present in the shared asset catalog,
  not just in particles_demo's `assets.ron`.

## Acceptance criteria

- `SpawnEffect(key: "shared/fire/campfire")` in any project works without a local
  definition in that project's `assets.ron`
- `SharedEffect(base: ..., overrides: ...)` correctly merges: unchanged fields from base,
  overridden fields from `overrides`
- Asset checker reports a clear error for an unknown `base` key
- particles_demo uses at least 3 shared effects and renders identically to the inline versions
- New projects can use all 10 initial shared effects with zero local effect definitions
