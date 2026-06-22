# Asset Requests — 3rd Person Game Demo (Greywatch)

> Handoff document between world design and asset production.
> Derived from `design/world_design.md` (v1, 2026-06-22). Requests below cover only
> what the Greywatch design genuinely needs and that does **not** already exist in
> `assets/shared/` or the project catalog.
>
> **Before requesting anything new, the design first reuses the shared library.** The
> shared pool is unusually rich (fountain, braziers, lanterns, lamp_post, fences,
> gate, bridge, statue_base, logs/firewood, rocks, lily_pads, trunk_with_branches,
> stone archways, greek columns, balusters, banner, candle holders, key_01, scrolls,
> coin piles, trophy; wizard / ghost / ghost-skull / orc creatures; a deep footstep +
> combat + fire SFX set; and stylized stone/bark/wood/marble PBR texture sets). Most
> of the village and all four monster zones can be dressed from those assets with RON
> transform/material/motion overrides.
>
> The genuine gaps fall into four buckets: **(A) building structures** (no houses,
> walls, roofs, or a market stall exist — the village currently reads as props on
> flat ground), **(B) graveyard signature props** (no grave markers / headstones),
> **(C) looping ambient audio** (only a single music bed exists; no wind, fire-crackle
> loop, marsh, or undead-moan ambience), and **(D) two atmosphere particle effects**
> the ruins climax wants.
>
> Status legend: `Requested` → `In Progress` → `Done`.

---

## Reuse-first notes (NOT requests — adapt existing assets via RON)

These design needs are satisfiable today with RON overrides; no new art. Recorded
here so production does not accidentally model them.

- **Forge weapon-rack display** — reuse `sword.glb`, `saber_01.glb`, `bronze_age_sword.glb`, `battle_axe.glb`, `halberd.glb` as static wall/anvil-side props with scale/rotation overrides. No new model.
- **Founder's statue (village + ruins)** — reuse `statue_base_01.glb`. Village copy gets a tilt (`rotation_euler_deg`) + grey-stone material to read "broken/leaning"; ruins copy stays upright. No new model.
- **Seal Door structure** — assemble from `stone_archway_01.glb` (or `_02/03/04`) + `wooden_gate_01.glb` reskinned with a cold stone material tint + `rock_0x` buttresses. The `key_01.glb` model already exists for the Old Key item icon/world prop. No new model.
- **Ruins columns / approach** — reuse `greek_column_01/02.glb` and `stone_archway_0x.glb` (cracked, partially toppled via rotation) + `iron_brazier_01.glb` relit. No new model.
- **Scout's-satchel breadcrumbs** — reuse `chest_02.glb` (closed/empty) + `scroll_01.glb` + `compass_pouch.glb` clustered near the marsh/woods. No new model.
- **Dead reeds / dry creek dressing** — reuse `lily_pad_01/02.glb` (dead-tinted, low-saturation green-brown material) + `log_01..03.glb` driftwood + `rock_01..04.glb`. No new model.
- **Spider-cocoon hint** — improvise from `log_01.glb` wrapped silhouette + a desaturated material; acceptable for v1. A bespoke cocoon is **Low** priority polish (filed below).
- **Sorcerer mini-boss** — reuse `wizard.glb` (on disk) as `enemy_sorcerer` prefab; reuse `respawn_glow` / `magic_04` / `magic_05` particles as cast visuals. Lesser spirits reuse `ghost.glb` / `ghost-skull.glb`. No new model. (A dedicated cast telegraph effect is filed below as Medium.)
- **Inn fire / forge fire glow** — reuse `iron_brazier_01.glb` + existing flame particle approach (see `particles_demo` `campfire_fire`). No new model; a looping crackle **audio** bed is requested below.

---

## A. Building Structures

### Village House (frontier cottage)
- **Type:** 3D model
- **Priority:** High
- **Status:** Requested
- **Needed for:** Greywatch village (the hub) — currently has no buildings, so the "warm, huddled, defended place" reads as scattered props on flat sand.
- **Description:** A small one-storey frontier dwelling: timber-frame walls, thatched or plank roof, a doorway and one or two shuttered windows. 3–4 of these clustered around the well form the village. Static prop (no interior needed for v1); the player walks between them, not into them.
- **Style direction:** Stylized hand-painted, chunky silhouette. Warm worn-wood walls (sienna/ochre, matching `Stylized_Wood_Planks_003`), earthy thatch roof (warm ochre, matching `Stylized_Thatched_Roof_003`). Threadbare and frontier-poor — patched walls, a leaning chimney — not a tidy storybook cottage. Roughly 4–5 m wide, 3 m to eaves (about 2.5× player height at the ridge).
- **Reference:** Should sit in the same world as `tiered_fountain_01.glb` and `wooden_gate_01.glb`; match the thatch/plank PBR sets already in `assets/shared/textures/`.
- **Suggested path:** `assets/shared/models/buildings/cottage_01.glb` (create `buildings/` subfolder)
- **Notes:** Web-budget polycount (keep low — painted detail over geometry). Bake AO/shadow into albedo per the art-style guide. Provide 2 roof or wall variants if cheap, so the village does not look copy-pasted. No animation. Generate an AVIF preview and run `--check` after adding.

### Market Stall
- **Type:** 3D model
- **Priority:** High
- **Status:** Requested
- **Needed for:** Greywatch — Edrin the merchant's shop location. The design currently improvises this from "a crate/log counter," which reads as nothing in particular.
- **Description:** An open-fronted timber market stall with a slanted cloth awning and a flat counter the merchant stands behind. Anchors the economy beat and gives the merchant a believable place to be.
- **Style direction:** Stylized hand-painted. Weathered wood frame (warm brown), faded awning cloth in a muted accent (dusty teal or rust — the one "outsider-wealthy" colour note tying to the merchant's silver). Should feel slightly better-kept than the houses (the merchant has coin). Counter height readable against a ~1.8 m character. About 2.5–3 m wide.
- **Reference:** Awning colour should read as a deliberate accent against the village's warm-brown wood, the way `mat_silver` marks the merchant as an outsider.
- **Suggested path:** `assets/shared/models/buildings/market_stall_01.glb`
- **Notes:** Low poly, painted detail. No animation. Counter surface flat enough to place small props (potion bottles, coin pile) on top via RON.

### Palisade Wall Segment
- **Type:** 3D model
- **Priority:** Medium
- **Status:** Requested
- **Needed for:** Greywatch palisade / the safe-vs-unsafe threshold (Zone Layout, z ≈ −2).
- **Description:** A tileable sharpened-log palisade wall segment that repeats along the village perimeter and frames the gate. The design currently leans on `rounded_picket_fence.glb` / `walking_trail_fence.glb`, which read as garden fencing, not a frontier defensive line — undercutting the "holding the line" core theme (world-building question 11).
- **Style direction:** Stylized hand-painted. Vertical lashed timber stakes, pointed tops, rough lashing rope detail. Warm-to-grey weathered wood, leaning toward grey at the exposed tops. Chunky, defensive, slightly battered (a wall that is *failing* to hold). About 2–3 m wide per segment, ~2.2 m tall (just above eye line), so it actually reads as enclosure.
- **Reference:** Must read as a heavier, defensive sibling of `rounded_picket_fence.glb` — same world, more menace. Pairs with `wooden_gate_01.glb` at the gate.
- **Suggested path:** `assets/shared/models/buildings/palisade_segment_01.glb`
- **Notes:** Tileable end-to-end with no visible seam when repeated. Provide a straight segment; a corner piece is nice-to-have (Low). Low poly. No animation.

---

## B. Graveyard Props

### Grave Marker Set (headstones)
- **Type:** 3D model
- **Priority:** High
- **Status:** Requested
- **Needed for:** The Graveyard zone (NE, Tier 3) and the two lost-scouts grave markers referenced in the design. There is currently **no** grave/headstone model in the shared pool, so the village's dead-ground has no signature prop.
- **Description:** A small set (2–3 variants) of weathered grave markers: a simple upright slab headstone, a leaning/cracked one, and optionally a low rounded fieldstone. These define the graveyard's identity and double as the two named scout graves (Tilly's brother, the lost scouts).
- **Style direction:** Stylized hand-painted cold stone (blue-grey/slate, matching the graveyard's "cold grey-green" palette and `Stylized_Cliff_Rock_005`/`Stylized_Rocks_003` hue family). Painted moss accent at the base (earned green accent per art guide), faint carved markings. Chunky silhouette readable at distance. One variant clearly leaning/broken to echo the "broken salt-lines / disturbed graves" fiction. About 0.8–1.1 m tall.
- **Reference:** Cold-stone sibling of `statue_base_01.glb`; share the rock-PBR hue family so graveyard, ruins, and statue all feel quarried from the same world.
- **Suggested path:** `assets/shared/models/props/grave_marker_01.glb`, `grave_marker_02.glb` (and optional `_03`)
- **Notes:** Low poly. No animation. Bake the moss/AO into albedo. Should look correct under the scene's golden directional light *and* in the colder graveyard lighting.

---

## C. Audio — Ambient Beds

> The thematic palette specifies a **temperature gradient of sound** (village quiet/safe →
> field tense → ruins ritual-dread). Today only `bg-music-balance.mp3` exists, plus
> one-shot combat/fire SFX. These looping ambient beds are what make each zone *sound*
> distinct. **All four depend on zone-based audio switching being supported by the
> engine — flagged in `world_design.md` Open Question 2 and `claude_suggestions.md`.**
> If zone audio is unsupported, only the village bed is usable as a global replacement
> ambience; the others drop to Low until the feature lands.

### Wind Ambience Loop (field / cold zones)
- **Type:** audio
- **Priority:** Medium
- **Status:** Requested
- **Needed for:** Spider Woods, Graveyard, and Ruins approach — the "colder, exposed" zones outside the palisade.
- **Description:** A seamless looping low wind bed, lonely and exposed, to underscore the drop in warmth once the player crosses the gate. Distinct from the village's quiet safety.
- **Style direction:** Low, steady wind with occasional gusts; no melody. Should sit *under* `bg_music` without fighting it. Sombre, isolating — "the warmth is behind you."
- **Reference:** Should layer cleanly beneath `bg-music-balance.mp3`; quieter and emptier than the village bed.
- **Suggested path:** `assets/shared/audio/ambient/wind_loop.ogg` (create `ambient/` subfolder)
- **Notes:** Seamless loop, no audible seam at the wrap point. Keep file small for web (mono is fine for ambience; OGG/MP3 preferred over WAV for size). Target a long-ish loop (20–40 s) so repetition is not obvious.

### Fire Crackle Loop (village hearth / forge / braziers)
- **Type:** audio
- **Priority:** Medium
- **Status:** Requested
- **Needed for:** The Inn Fire and Forge in Greywatch — the warm social corner. Existing `fire/` assets are **one-shots** (`light-fire-sound`, `fire-ignition`), not a sustained loop.
- **Description:** A seamless looping campfire/brazier crackle for the warm village hearth, reinforcing "the village holds the light." Positional if the engine supports per-emitter spatial audio; otherwise a soft global village-layer bed.
- **Style direction:** Gentle, close, comforting crackle and pop — not an aggressive roar. The auditory equivalent of the warm amber lantern pools.
- **Reference:** Same fire family as the existing `shared/audio/fire/` one-shots, but sustained and loopable.
- **Suggested path:** `assets/shared/audio/ambient/fire_crackle_loop.ogg`
- **Notes:** Seamless loop. Low volume by default (set in `assets.ron` `volume:`). Web-friendly compressed format.

### Marsh Ambience Loop (Snake Marsh)
- **Type:** audio
- **Priority:** Low
- **Status:** Requested
- **Needed for:** Snake Marsh (SW, Tier 1) — the reedy, drained creek bed.
- **Description:** A quiet looping marsh bed: faint reed rustle, occasional water drip, low insect hum. Sets the "shallow end of danger" tone and contrasts the dry, exposed field.
- **Style direction:** Damp, low, slightly uneasy but not threatening (threat comes from the snake aggro stinger, not the ambience). Sparse.
- **Reference:** Pairs with the existing `footsteps-water/` SFX; complements rather than overlaps the wind loop.
- **Suggested path:** `assets/shared/audio/ambient/marsh_loop.ogg`
- **Notes:** Seamless loop, small file. Low priority — the wind loop can stand in for v1 if needed.

### Undead Moan / Ruins Drone Loop
- **Type:** audio
- **Priority:** Low
- **Status:** Requested
- **Needed for:** Graveyard and Ruins of the Seal — the dread payoff zones.
- **Description:** A low, slow drone with occasional distant moans, for the cursed dead-ground and the sealed shrine. Carries the "ritual / containment broken" dread that is the climax's emotional core.
- **Style direction:** Sub-heavy, slow-moving, sickly. Sparse distant vocal moans (not constant — silence between them does the work). Should *spike* in intensity feel during the sorcerer fight (can be a second, more active variant or handled by raising volume in RON).
- **Reference:** Tonally the opposite of the village fire/wind beds — this is where the temperature gradient bottoms out.
- **Suggested path:** `assets/shared/audio/ambient/undead_drone_loop.ogg`
- **Notes:** Seamless loop. Low priority for v1 (the field wind bed degrades gracefully here), but the highest mood-per-effort once zone audio exists. Web-friendly format.

---

## D. Particle Effects

### Sorcerer Cast Telegraph
- **Type:** particle effect (RON `EffectDef` in `assets.ron`)
- **Priority:** Medium
- **Status:** Requested
- **Needed for:** The Sorcerer mini-boss (Ruins, Tier 4 climax). The design notes the fight should *telegraph* casts; the existing `respawn_glow` reads as a friendly cyan shimmer, wrong for a hostile windup.
- **Description:** A charge-up burst played at the sorcerer's hands/position before a damaging cast, giving the player a readable "incoming" tell. Authored entirely in RON using existing particle sprites — **no new texture or model needed**, so this is effectively a RON-authoring task, not art production.
- **Style direction:** Sickly green/violet to match the ruins' "spell-light" palette (e.g. `color_start` ~ (0.4, 0.95, 0.3) into (0.55, 0.05, 0.85)). A gathering/converging motion (use `Ring` or `Arc` emitter with `EaseIn` velocity curve so particles rush inward, reading as "charging"). Additive blend for glow. Slow enough to read per the art-style guide (no fast strobe).
- **Reference:** Reuse `particle/magic_04` / `particle/magic_05` sprites already in `assets.ron`; contrast it deliberately against the *friendly* cyan `respawn_glow`.
- **Suggested path:** add `"sorcerer_cast"` to `assets/projects/3rd_person_game_demo/assets.ron` `effects:` (or `shared/textures/particles/` only if new sprites are ever needed — they are not).
- **Notes:** Pure RON; can be authored by the developer/designer. Warm up the pipeline on `scene.ready` like other additive effects. No new texture required.

### Ground Mist / Fog (ruins + graveyard)
- **Type:** particle effect (RON `EffectDef`) — possibly new sprite
- **Priority:** Low
- **Status:** Requested
- **Needed for:** Ruins of the Seal and Graveyard — low-lying dread atmosphere.
- **Description:** A slow, low ground-hugging mist drifting across the cursed ground, deepening the "coldest, most deliberate space" feel of the ruins. Atmosphere only, no gameplay effect.
- **Style direction:** Desaturated cold blue-grey, very low opacity, slow drift (`turbulence` low, `gravity` ~0, `EaseOut` or `Linear`). Large soft quads near ground level (`offset` low Y, `Disc` or `Line` emitter). Must stay subtle — readable, not a screen-filling fog that kills the painted look.
- **Reference:** Can likely reuse `particle/smoke_01` / `particle/smoke_02` (as `death_poof` does). Only request a new soft-fog sprite if smoke reads too puffy at large scale.
- **Suggested path:** add `"ground_mist"` to project `assets.ron` `effects:`; new sprite (if needed) at `assets/shared/textures/particles/fog_soft_01.png`.
- **Notes:** Low priority polish. Try the existing smoke sprites first (zero new art). Watch the WASM particle budget — keep `particle_count` modest and do not run many emitters at once.

---

## Summary & Priority Flags

**Filed: 9 requests** (3 building structures, 1 graveyard prop set, 4 audio beds, 2 particle effects — 10 items, with the cocoon noted inline as Low polish). Plus a "reuse-first" block documenting ~9 needs already covered by existing assets via RON.

**High priority — these block populating the scene as designed:**
- **Village House (cottage_01)** — without it the hub is props on flat sand, not "Greywatch." Blocks the warm/safe emotional anchor that the entire progression contrasts against.
- **Market Stall** — Edrin's shop has no believable location; the economy beat falls flat.
- **Grave Marker Set** — the Graveyard (Tier 3) and the named scout graves have no signature prop; the zone is currently indistinguishable from open ground + a statue.

**Medium priority** (quality/atmosphere): Palisade Wall Segment (sells the "holding the line" theme; `rounded_picket_fence` is a usable stopgap), Wind ambience loop, Fire crackle loop, Sorcerer cast telegraph (RON-only).

**Low priority** (polish, several degrade gracefully): Marsh loop, Undead drone loop, Ground mist, spider cocoon.

**Cross-cutting dependency to flag:** all four **audio ambient beds** depend on **zone-based audio switching** (Open Question 2 in `world_design.md`; needs an entry in `planning/claude_suggestions.md`). If that engine feature is absent, only the wind or fire loop is usable as a single global ambience and the per-zone sound gradient cannot be delivered in v1. The **High-priority blockers are all art**, not engine — they can proceed immediately regardless of the audio feature status.
