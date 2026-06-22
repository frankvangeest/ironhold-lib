# World Design — 3rd Person Game Demo

> Status: v1 design pass, 2026-06-22. Grounded in the project's current prefabs,
> items, stats, dialogue, and the shared model library. Nothing here requires
> engine code changes unless explicitly flagged in **Open Questions** /
> `planning/claude_suggestions.md`.

---

## Vision & Emotional Core

**Greywatch** — a struggling frontier village on the edge of land the dead won't
stay buried in. This is a **small-town survival RPG**: the village is the one warm,
lit, defended place in a cold landscape, and every direction out of it leads
somewhere worse. The player is not a chosen hero arriving to a grateful kingdom;
they are an armed traveller the villagers are quietly, desperately hoping will do
what their two lost scouts could not.

The existing dialogue already establishes this tone perfectly ("We've lost two
scouts already", "I'd go myself but someone has to watch this post"). The world
design extends that single guard's voice into a coherent place.

**The one-line pitch:** *The village holds the light; you carry it outward.*

**Emotional throughline of a play session:**
1. **Safety & obligation** (village) — warmth, NPCs who need you, a clear ask.
2. **Tension & competence** (near zones) — first real danger, but winnable; you learn the combat verbs.
3. **Dread & resolve** (the ruins) — the source of the corruption; the payoff.
4. **Return & relief** (back to village) — you are richer, the elder pays out, the light feels earned.

---

## World Building Foundations (the 12 questions)

1. **What does the average person do all day?** Subsistence work — the village
   was a waystation/trade-post before the dead rose. Now people barricade, ration,
   tend the well, repair the palisade, and wait for travellers. The blacksmith still
   works because weapons are the one export everyone needs.
2. **Who holds power, and how do they keep it?** The **Village Elder** (Maren),
   by being the last person old enough to remember the land before the curse and
   the only one with a sack of gold to spend. Power here is moral authority +
   the last of the coin, not force.
3. **What history does everyone know?** Three nights ago a sorcerer was seen near
   the northern ruins; since then the dead walk. (Established in dialogue — canon.)
4. **What history is known only by a few?** The ruins were a shrine the village's
   founders deliberately sealed generations ago. The Elder knows this. The "old key"
   the merchant is selling is a founder's key — and the merchant doesn't fully know
   what he's holding. (See **The Old Key** section.)
5. **What do people believe?** A folk-faith that the dead must be *kept* buried —
   ritual, weighted graves, salt lines. The sorcerer broke a taboo, not just a law.
6. **What are the rules?** Don't go north after dark. Don't disturb the graves.
   Pay the blacksmith fairly. Travellers are welcome at the well but watched.
7. **What is scarce?** Safety, gold, able bodies, and *daylight*. Potions are
   precious (the merchant rations them). This justifies the economy already in items.ron.
8. **How do people travel and communicate?** On foot, along the old stone road and
   the bridge to the north. No fast travel in-fiction — distance is felt.
9. **What does status look like?** The merchant's fine silver-grey clothing
   (`mat_silver`) marks him as outsider-wealthy. Villagers are plainer. The Elder
   has presence, not finery.
10. **What is the cost of conflict?** Real. Two scouts dead. The player can die.
    Enemies respawn (the curse keeps replenishing them) — the land does not heal
    until the source is dealt with.
11. **What makes this different from generic fantasy?** The *containment* framing:
    this isn't "kill the evil," it's "re-seal what was always here." The threat is
    a broken quarantine, not an invading army. The village's job was always to hold
    a line, and they're failing.
12. **The grounding constraint:** everything above is buildable with the prefabs,
    items, and shared models already present. No new mechanics are required for v1.

---

## Zone Layout (spatial map)

Coordinate convention matches the existing scene: **player spawns at +Z (south),
−Z is north, +X is east, −X is west.** Ground is currently 100×100; the layout
below fits comfortably inside that footprint, with the ruins pushing the far north
edge. Distances are in world units (≈ 1 unit ≈ 1 m, player ~1.8 m tall).

```
                          NORTH  (−Z)  — danger increases with distance from village
        ┌─────────────────────────────────────────────────────────┐
        │   [RUINS OF THE SEAL]  the sorcerer, undead, the key door  │  z ≈ −38..−48
        │            ▲  approached via the old stone bridge          │
        │            │                                               │
   W    │  [SPIDER   │            [GRAVEYARD]                         │  z ≈ −20..−32
  (−X)  │   WOODS]   │         restless dead / zombies               │   (+X)  E
        │  z −18..−30│         z −18..−30,  x −6..+10                 │
        │  x −22..−8 │                                               │
        │            ╞══ stone bridge over the dry creek ══╡         │  z ≈ −14
        │                                                            │
        │              [SNAKE MARSH / dry creek bed]                 │  z ≈ −6..−16
        │              low ground SW,  x −16..−2                     │
        │                                                            │
        │   ════════════ palisade / fence line ════════════         │  z ≈ −2
        │   ┌──────────────  GREYWATCH  ──────────────┐              │
        │   │  well · blacksmith · merchant · inn fire │  SAFE ZONE   │  z ≈ +2..+12
        │   │            player spawns here            │              │
        │   └──────────────────────────────────────────┘             │
        └─────────────────────────────────────────────────────────┘
                          SOUTH  (+Z)  — the road home / world edge
```

**Landmarks (player-legible navigation anchors):**
- **The well + fountain** at village center (`tiered_fountain_01`) — the heart, the spawn-adjacent safe point.
- **The palisade gate** (`wooden_gate_01` + `rounded_picket_fence` / `walking_trail_fence`) — the threshold between safe and unsafe. Crossing it is the emotional "leaving home" beat.
- **The stone bridge** (`garden_bridge_01`) over the dry creek at z ≈ −14 — the single chokepoint to the north. The guard's dialogue already references "the old stone bridge."
- **The graveyard's leaning statue** (`statue_base_01`) — a broken founder's monument, marks the dead-ground.
- **The ruins' sealed door** — the destination; built from `statue_base_01` + rocks + braziers (see Ruins zone).

---

## Village Design — Greywatch

**Theme:** a fortified frontier hamlet at dusk that refuses to go dark. Lantern-lit,
huddled, warm against a cold landscape. Lived-in but threadbare.

**Atmosphere / palette:** warm amber pools of lantern light against the existing
late-afternoon gold directional light (keep the current `main.scene.ron` lighting —
it already reads as "golden hour on the frontier"). Inside the palisade: warm woods
(`0.45, 0.28, 0.12` already used on the loot platform), firelight, the cool silver
of the merchant as the one "outsider" accent. The contrast the player should feel:
**inside the fence = warm and close; outside = open, exposed, colder.**

**Audio mood:** keep `bg_music` (`bg-music-balance.mp3`) as the village/ambient
bed. Village should feel *quieter and safer* than the field — see Open Questions on
zone-based audio. NPC voice tone: tired, plain-spoken, grateful-but-guarded.

### Key locations

| Location | Built from (existing assets) | Purpose |
|---|---|---|
| **The Well** (center) | `tiered_fountain_01` | Visual heart; natural gather point; safe-feeling landmark near spawn |
| **The Forge** | `prop_anvil` (already in scene) + `iron_brazier_01` + `firewood_01/02` + `sword`/`saber_01` props as wall display | Blacksmith's station; ties the Iron Sword to a maker |
| **The Market Stall** | `merchant_vendor` (existing) + a crate/`log_01` counter | Shop; potions, sword, the old key |
| **The Inn Fire** | `iron_brazier_01` + `log_01/02/03` as benches + `lantern_01/02` | The warm social corner; where the questgiver NPC stands |
| **The Palisade & Gate** | `rounded_picket_fence`, `walking_trail_fence`, `wooden_gate_01` | The safe/unsafe threshold |
| **The Founder's Statue** | `statue_base_01` | Lore object; foreshadows the ruins' identical (broken) statue |

### Named NPCs (3–5)

All reuse `character_male` / `character_female` + `mat_silver` or plain material.
Each is a prefab clone of `friendly_npc_male` / `merchant_vendor` with its own
`dialogue` file and position. **No new model work needed for the NPCs themselves.**

1. **Maren, the Village Elder** *(friendly NPC, female — `character_female`)*
   - **Role:** quest-giver and gold-payer. Stands by the well.
   - **Personality:** weary authority; speaks in short, certain sentences; carries the
     village's grief without dramatizing it.
   - **Offers:** the **main quest** ("re-seal the ruins"), the gold reward the guard's
     dialogue already promises, and the secret half of the lore (the ruins were
     *sealed*, not built by enemies). Dialogue hook: gates the reward, names the key.

2. **Halvard, the Guard** *(friendly NPC, male — already exists as `npc_01` / `friendly_npc_male`)*
   - **Role:** tutorial voice + threat briefing. **Keep his current `npc_intro.dialogue.ron` — it is already on-tone.** Just reposition him to the gate.
   - **Personality:** dutiful, blunt, can't leave his post. (Already written.)
   - **Offers:** directions, threat warnings, the "investigate the ruins" hook. Position him *at the gate* so his "stay sharp" line lands as the player crosses the threshold.

3. **Brann, the Blacksmith** *(friendly NPC, male — `character_male`, darker/plainer material)*
   - **Role:** flavour + soft progression gate. Stands at the forge by the anvil.
   - **Personality:** gruff, practical, secretly frightened; channels fear into work.
   - **Offers:** dialogue that *explains the Iron Sword* ("Last good steel I've got.
     Buy it from Edrin's stall — I'm too busy keeping the watch's blades sharp").
     This routes the player to the merchant and makes the sword feel earned. Could
     also be a second, cheaper weapon vendor later.

4. **Edrin, the Merchant** *(`merchant_vendor`, already exists)*
   - **Role:** economy. **Keep his current stock** (potions, iron_sword, old_key).
   - **Personality:** transactional, a little too smooth, an outsider who stayed when
     others fled — because the curse is *good for a potion-seller*. Mild moral grey.
   - **Offers:** the shop (existing). Add a dialogue layer (currently he's interact→shop
     only) so he can sell the old key *with a line that misframes it* ("Found it in
     the dead scout's pack. Opens something up north, I'd wager. Yours for 150.").

5. **(Optional 5th) Tilly, the Inn-keeper's child** *(`character_female`, small scale)*
   - **Role:** emotional anchor; makes the stakes personal and human.
   - **Personality:** unafraid in the way only children are; asks the player to "bring
     the scouts home."
   - **Offers:** no mechanics — pure tone. A short dialogue that recontextualizes the
     graveyard ("My brother's out there. Mama says don't look north."). Reuse the
     dialogue system; no new tech.

> **Decision:** NPC count for v1 = 4 named (Maren, Halvard, Brann, Edrin), Tilly as
> a stretch goal. Rationale: 4 covers quest-give, tutorial, economy-context, and
> economy; Tilly is high emotional value but lowest mechanical necessity. Logged below.

---

## Monster Zone Design

Difficulty tiers are tuned to the **existing prefab stats** (snake 50 HP / fast,
spider 75 HP / fastest+widest, zombie 120 HP / slow+hard-hitting) and the player's
existing action bar (Attack −30, Heavy −60, Mana Blast −15, Poke −1, Heal +25).

### Zone 1 — The Snake Marsh (SW, dry creek bed) · TIER 1 (tutorial danger)
- **Theme:** a drained, muddy creek bed just outside the palisade. Low, close,
  reedy. The "shallow end" of danger — the first thing the player meets on foot.
- **Enemies:** 2–3 `enemy_snake`. Fast but fragile (50 HP, dies to two Attacks).
  Ground-level ambush teaches the player to *watch the ground* and to *select a target*.
- **Why first:** lowest HP, lowest single-hit threat; ideal for learning the
  click-select → press 1 loop without dying. Reachable without crossing the bridge.
- **Loot/reward:** a `chest_01`-style chest (health potions) tucked behind a `log_01`
  — rewards exploration, refills before the player commits north.
- **Environmental storytelling:** a dropped scout's satchel (a `chest_02` or
  `firewood`/prop cluster) near the creek — the first hint the scouts came this way.
  Built from: `rock_01..04`, `log_01..03`, `lily_pad_01/02` (dry/dead), reeds (foliage).

### Zone 2 — The Spider Woods (NW) · TIER 2 (the pressure zone)
- **Theme:** a dim, overgrown treeline west of the bridge. Claustrophobic, things
  move at the edge of vision. The existing spider purple-glow theme reads great here.
- **Enemies:** 2–3 `enemy_spider`. Fastest chase (5.0) and widest detection (10) of
  any enemy — they *come to you*, so this zone teaches kiting, Heavy Strike timing,
  and managing multiple aggro. 75 HP each.
- **Difficulty:** the spike between marsh and graveyard. Survivable with potions;
  punishing if the player aggros all three.
- **Loot/reward:** `loot_display` (already a prefab!) on a small clearing — a
  literal raised platform with a chest, guarded by `spider_02`. This *already exists
  in the current scene* as a loot guardian; the design just relocates it into a themed zone.
- **Environmental storytelling:** `trunk_with_branches_01` + dense rocks; a wrapped
  cocoon shape (improvise from `log` + foliage) hinting the second scout's fate.

### Zone 3 — The Graveyard (NE) · TIER 3 (the grind / proof zone)
- **Theme:** the village's dead-ground, where the curse is thickest. Leaning
  founder's statue (`statue_base_01`), salt-lines broken, graves disturbed.
- **Enemies:** 2–4 `enemy_zombie`. Slow (1.5 chase) but 120 HP and hit for 15 every
  4 s — a war of attrition. Teaches resource management (Heal on slot 5, mana economy)
  and rewards the Iron Sword / Heavy Strike investment.
- **Difficulty:** the highest *sustained* threat before the boss. The zone the
  guard's "proof that the sorcerer has been dealt with" quest implicitly runs through.
- **Loot/reward:** gold-bearing chests; this is where the player earns the coin to
  *afford the old key* (150g) if they didn't start with enough. Closes an economy loop.
- **Environmental storytelling:** `statue_base_01` as a broken founder monument —
  visually rhymes with the *intact-then-broken* seal at the ruins. Scattered
  `iron_brazier_01` (burned-out funeral fires). Two grave markers for the lost scouts.

### Zone 4 — The Ruins of the Seal (far N, across the bridge) · TIER 4 (climax)
- **Theme:** the source. An ancient sealed shrine the founders built to *keep
  something down*. The sorcerer has cracked it open. This is the darkest, coldest,
  most deliberate space — the payoff for the whole journey.
- **Enemies:**
  - A gauntlet of mixed undead (zombies) on the approach.
  - **The Sorcerer** — a *new* mini-boss using the **`wizard.glb`** shared model
    (already available!). High HP, casts (reuse `respawn_glow`/magic particle effects
    as "spell" telegraphs). This is the one genuinely new prefab worth building.
  - Optional: `ghost.glb` / `ghost-skull.glb` as lesser spirits for atmosphere.
- **Loot/reward:** the **Seal Door** — what the **Old Key** opens (see below). Behind
  it: the quest payoff and a high-value chest (gold + a trophy item using `trophy.glb`).
- **Environmental storytelling:** an *intact* founder's statue (mirrors the broken
  graveyard one) flanking a stone door (`statue_base_01` + `rock_0x` + `wooden_gate_01`
  reskinned as stone). Braziers relit by the sorcerer. The seal: a cracked stone
  circle on the ground (a decal or scaled primitive disc).

---

## The Old Key

The `old_key` (items.ron, quest-tagged, sold by the merchant for 150g) is the
**Founder's Key to the Seal Door** at the ruins.

**Fiction:** Generations ago, Greywatch's founders sealed the northern shrine and
split its key — one half lost, one half kept. The lost half ended up in a dead
scout's pack, which the merchant Edrin scavenged and now sells without understanding
its true purpose. The sorcerer breached the *outer* ruins but cannot open the *inner*
seal without this key — which is exactly why the dead still rise (he's working on it,
and failing). **The player who buys/finds the key can do what the sorcerer cannot:
reach the seal and close it.**

**Mechanical role (buildable now):**
- The **Seal Door** at the ruins is an `interactable` prop. Its interaction is
  *gated on the player holding `old_key`* in inventory.
- Two acquisition paths, supporting player choice:
  1. **Buy it** from Edrin for 150g (existing merchant stock) — the gold-economy path,
     which the graveyard zone funds.
  2. **(Stretch) Find it** as loot in the Spider Woods or on the dead scout — the
     exploration path, which makes the woods a meaningful detour.
- Using the key at the door fires the climax: opens the seal, lets the player confront/
  finish the sorcerer encounter, and completes the elder's quest for the gold reward.

> **Engine check needed:** does the current build support an interactable that is
> *conditional on an inventory item*? If not, this is a small, high-value feature —
> flagged in Open Questions and `claude_suggestions.md`.

---

## Progression Flow

A natural, difficulty-ramped loop that needs no hand-holding:

1. **Spawn in Greywatch (safe).** Warm, lit, NPCs nearby. Player learns movement
   and camera in zero-threat space.
2. **Talk to Halvard at the gate.** Existing dialogue delivers the threat + the
   "investigate the ruins" hook + directions ("north-east, past the old stone bridge").
3. **Visit the merchant & blacksmith.** Buy a potion or the Iron Sword with starting
   gold (200). Brann's dialogue routes the player to the stall. Learn the economy.
4. **Cross the gate (the emotional threshold).** First step out of the warm zone.
5. **Snake Marsh (Tier 1, SW).** First fight. Cheap, winnable, teaches target-select →
   attack. Find a refill chest + the first scout-trail breadcrumb.
6. **Spider Woods (Tier 2, NW).** Pressure ramps. Multi-aggro, kiting, the
   `loot_display` reward. (Optional key-find path here.)
7. **Graveyard (Tier 3, NE).** Attrition fight vs. zombies. Earn the gold to afford
   the old key if not already owned. The fiction's "proof" zone.
8. **Buy/confirm the Old Key**, cross the **stone bridge** north.
9. **Ruins of the Seal (Tier 4).** Undead gauntlet → use the key on the Seal Door →
   the Sorcerer encounter → close the seal.
10. **Return to Maren.** Claim the reward. The world feels (and could literally be,
    via a state flag) calmer. Relief + earned warmth.

The compass directions are deliberately spread so each zone is a distinct *trip*
from the village, and the bridge gates the final two tiers behind a single memorable
landmark.

---

## Scene Split Recommendation

**Recommendation: keep ONE gameplay scene for v1 (`main.scene.ron`), expanded.**

Reasoning:
- The whole emotional design depends on *continuity of space* — feeling the distance
  from the warm village to the cold ruins. Loading screens between zones would break
  the "the light is behind you" tension that is the core of the pitch.
- The 100×100 ground already fits the entire layout above. No streaming needed.
- The state machine is already wired around a single `main` scene; multiplying scenes
  multiplies the `spawning_*` / `playing` plumbing for no player benefit yet.
- Enemy counts (≈ 10–14 total across zones) are within the existing demo's spawn budget
  ballpark; watch the WASM frame-time reviewer if it climbs past ~20 active NPCs.

**When to split (post-v1):** if the Ruins become a distinct *interior* (a sealed
chamber behind the door) with its own lighting (cold, dark, candle-lit vs. the
field's gold). At that point the Seal Door interaction becomes a `LoadScene` to
`scenes/ruins_interior.scene.ron` — a clean, motivated cut. The engine supports it
(`PreloadScene` + `LoadScene` already used for pause/menu). Until the interior exists,
one scene is correct.

> **Decision:** single scene for v1; the Seal Door is the designated future
> scene-cut seam. Logged below.

---

## Immediate Next Prefabs / Assets Needed

Prioritized for a small team. Items 1–4 are RON-authoring only (no new art); items
5–6 use shared models already on disk.

1. **Village dressing prefab set (RON only).** New prefabs wrapping existing shared
   models: `prop_fountain` (`tiered_fountain_01`), `prop_brazier` (`iron_brazier_01`),
   `prop_lantern` (`lantern_01`), `prop_fence` (`rounded_picket_fence`),
   `prop_gate` (`wooden_gate_01`), `prop_bridge` (`garden_bridge_01`),
   `prop_statue` (`statue_base_01`), `prop_log_bench` (`log_01`). **Highest impact:**
   instantly turns the flat sandbox into a *place*. ~1 afternoon of RON.

2. **Three named-NPC prefabs (RON only).** `npc_elder_maren` (`character_female`),
   `npc_blacksmith_brann` (`character_male`, plain material), and a `dialogue` layer
   on `merchant_vendor`. Clones of existing friendly-NPC prefab with new `dialogue` files.

3. **Three new dialogue files (RON only).** `elder_maren.dialogue.ron` (quest + reward
   + secret lore), `blacksmith_brann.dialogue.ron` (routes to merchant, sword flavour),
   `merchant_edrin.dialogue.ron` (misframes the old key). Keep `npc_intro.dialogue.ron`
   as-is for Halvard.

4. **Seal Door prefab + key-gated interaction (RON; may need 1 engine feature).**
   `prop_seal_door` built from `statue_base_01` + `wooden_gate_01` + rocks. Needs the
   *item-gated interactable* check — see Open Questions / claude_suggestions.

5. **Sorcerer mini-boss prefab (uses `wizard.glb`, already on disk).** `enemy_sorcerer`:
   high HP stat template, a behavior file that telegraphs casts with existing magic
   particle effects, reuses NPC AI. The one "new enemy" worth the effort — it's the climax.

6. **Atmosphere creatures for the ruins (optional, uses `ghost.glb` / `ghost-skull.glb`).**
   `enemy_wisp` / non-combat `prop_ghost` for dread dressing in the ruins approach.
   Pure tone; low priority but high mood-per-effort.

> Note: `snake01.glb` and `spider01.glb` are currently untracked in git status —
> confirm they're committed before relying on them in a shared scene.

---

## Thematic Palette (quick reference for asset authors)

| Zone | Light / color | Material feel | Audio mood |
|---|---|---|---|
| **Greywatch** | warm amber lantern pools, golden directional (keep current) | warm worn wood, soft cloth, one silver accent (merchant) | quiet, safe; bg_music bed; gentle fire crackle |
| **Snake Marsh** | muted green-brown, low flat light | wet mud, dead reeds, grey driftwood | reeds, water-drip, low; tense on aggro |
| **Spider Woods** | dim, desaturated, purple accent (spider glow) | dark bark, dense rock, sticky web | sparse, skittering, sudden; claustrophobic |
| **Graveyard** | cold grey-green, long shadows | broken stone, cold iron, dead grass | wind, distant moans; sombre |
| **Ruins of the Seal** | coldest — deep blue shadow, sickly green/violet spell-light | ancient stone, cracked seal, relit braziers | low drone, ritual; spikes during the sorcerer fight |

The progression is a deliberate **temperature gradient**: warm amber at home →
colder and more saturated-toward-sickly the further north you go. The player should
feel the warmth drain out of the world as they approach the source.

---

## Open Questions

1. **Item-gated interaction** — Can an `interactable` (e.g. the Seal Door) be made
   conditional on the player holding a specific inventory item (`old_key`)? This is
   the linchpin of the main quest. If unsupported, flagged in claude_suggestions.
   *Workaround if unsupported:* gate the door on a GameVariable set when the key is
   bought (`buy_item:old_key` already fires an event we can hook), accepting that
   "buying" rather than "possessing" opens it.

2. **Zone-based / positional audio** — The palette above wants the village to *sound*
   safer than the field (different ambient beds per zone). Does the engine support
   triggering ambient music/SFX changes on entering a TriggerZone? If yes, this is
   pure RON. If no, v1 uses the single `bg_music` bed everywhere and we note the
   degraded experience.

3. **Quest state / reward payout** — Maren's gold reward implies tracking "sorcerer
   defeated." Can we set a GameVariable on the sorcerer's death event and gate a
   reward dialogue branch / `ModifyStat(gold)` on it? (Likely yes via existing
   event + SetVariable + dialogue, but needs confirmation that dialogue choices can
   be conditionally shown.)

4. **NPC scale for Tilly** — does the prefab system allow a per-instance scale on an
   Actor so a child reads as smaller? (Scene transform has `scale`; confirm it
   applies cleanly to rigged characters without breaking the collider.)

5. **5th NPC (Tilly)** — in or out for v1? Recommended: stretch goal. Awaiting Frank.

---

## Decision Log

- **Village named "Greywatch"** — *2026-06-22, confirmed (proposable).*
  What: the village/hub is named Greywatch. Why: "watch" reinforces the
  containment/holding-the-line theme (question 11); "grey" sets the cold, threadbare
  tone and contrasts the warm interior light. Trade-off: generic-leaning, but
  evocative and easy to remember. Status: proposed pending Frank.

- **Single gameplay scene for v1** — *2026-06-22, confirmed.*
  What: keep one `main.scene.ron`, expand it; do not split per-zone. Why: spatial
  continuity carries the core "leaving the light" emotion; fits the 100×100 ground;
  avoids state-machine plumbing churn. Seam for a future cut = the Seal Door → ruins
  interior. Trade-off: one big scene's spawn/particle budget must be watched on WASM.

- **Old Key = Founder's Key to the Seal Door** — *2026-06-22, confirmed.*
  What: the existing quest item unlocks the ruins' inner seal; the merchant sells it
  unknowingly. Why: gives the already-existing item a concrete world purpose, ties
  merchant + ruins + main quest into one loop, and explains *why the player can do
  what the sorcerer can't*. Trade-off: depends on item-gated interaction (Open Q1).

- **Compass-spread zones, bridge-gated climax** — *2026-06-22, confirmed.*
  What: Marsh SW (T1), Woods NW (T2), Graveyard NE (T3), Ruins far-N across the bridge
  (T4). Why: each zone is a distinct trip from the hub; difficulty maps to existing
  prefab stats (snake<spider<zombie<sorcerer); the bridge is a memorable single
  chokepoint gating the finale, matching Halvard's existing dialogue.

- **Reuse `wizard.glb` for the Sorcerer; reuse all existing enemy prefabs** — *2026-06-22, confirmed.*
  What: the only new combat prefab is the Sorcerer mini-boss (existing shared model).
  Why: maximizes world depth per unit of art effort for a small team; the existing
  snake/spider/zombie already form a clean three-tier ramp.

- **4 named NPCs for v1 (Maren, Halvard, Brann, Edrin); Tilly as stretch** — *2026-06-22, proposed.*
  What: the named-NPC roster. Why: covers quest-give, tutorial, economy-routing, and
  economy; Tilly adds emotional weight but no mechanics. Status: proposed pending Frank.

- **Keep Halvard's existing dialogue verbatim** — *2026-06-22, confirmed.*
  What: `npc_intro.dialogue.ron` is reused unchanged for the gate guard. Why: it is
  already perfectly on-tone and even names the bridge and ruins; rewriting would be
  churn. Reposition him to the gate so the "stay sharp" line lands on threshold-crossing.
