# Feature: Particle System v2 — 3. Bloom / Post-Processing in Scene RON

_Status: Blocked — see constraint below_
_Planned at: `ff085be` (2026-05-19)_
_Blocked at: `f46d462` (2026-05-23)_
_Part of: see `planning/features/particle_system_v2.md` for the full v2 overview_

## Blocking constraint — HDR is mandatory

Bevy 0.18's `Bloom` component is declared with `#[require(Hdr)]`. Inserting it on a
camera unconditionally enables HDR rendering — there is no opt-out. HDR rendering breaks
the WASM/WebGPU build: the scene renders as corrupted geometry (screenshot confirmed
2026-05-23). The engine's design principle is SDR-only, platform-consistent rendering;
this feature cannot be implemented with Bevy's built-in bloom pass without violating that
principle.

**What was tried:** full implementation was merged then reverted after visual confirmation
of the WASM breakage. The schema (`PostProcessingDef`, `BloomDef`), runtime camera
insertions, RON test, and `particles_demo` scene block were all reverted.

## If this is ever revisited

Two paths exist:

1. **Native-only bloom** — `#[cfg(not(target_arch = "wasm32"))]` guard on the `Bloom`
   insert. Web and native renders differ visually. Acceptable only if the engine formally
   adopts a "native-enhanced, web-baseline" policy for post-processing.

2. **Custom SDR bloom pass** — a WGSL post-process shader that extracts near-white pixels
   from the SDR framebuffer, blurs them, and composites back. Significant new work; needs
   performance profiling on WebGPU before enabling in production. Avoids the HDR
   requirement entirely.

Until one of these paths is chosen and budgeted, this feature stays in the icebox.

## Original design (preserved for reference)

`GameSceneV2` gains an optional `post_processing` block. When present, Bevy's built-in
bloom post-processing is enabled on the scene camera with the authored parameters.

```ron
post_processing: (
  bloom: (
    intensity: 0.25,
    low_frequency_boost: 0.45,
    high_pass_frequency: 0.85,
    threshold: 0.0,
    composite_mode: EnergyConserving,
  ),
),
```

The Bevy 0.18 struct fields (verified): `intensity`, `low_frequency_boost`,
`low_frequency_boost_curvature`, `high_pass_frequency`, `prefilter: BloomPrefilter {
threshold, threshold_softness }`, `composite_mode: BloomCompositeMode`, `scale: Vec2`.
