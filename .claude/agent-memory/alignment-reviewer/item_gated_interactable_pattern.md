---
name: item-gated-interactable-pattern
description: InteractableDef.requires_item + entity.interact_blocked — the canonical "gate an existing capability event on a designer-authored condition" recipe; single-construction-site win, dual CLI+runtime-warn diagnostics, and the two scoping asymmetries the runtime warn inherits
metadata:
  type: project
---

`feature/item_gated_interactable` (2026-09-14, verdict ALIGNED). Reference implementation for
**"gate an existing capability's event on a designer-authored condition"** — the cheapest kind of
capability extension to get right, and a template to copy.

**The 8 touchpoints (all present here):**
1. `schema/catalog.rs::InteractableDef.requires_item: Option<String>` — `#[serde(default)]`,
   struct already `deny_unknown_fields`, doc comment naming both the catalog it keys into and the
   event it changes.
2. `capabilities/interactable.rs::Interactable.requires_item` (component mirror).
3. **Single construction site**: `entity_spawner.rs::attach_prefab_features` is now the only place
   `Interactable {}` is built — so the [[prefab-marker-three-spawn-paths]] footgun does **not**
   apply to `interactable`/`inventory`/`dialogue`/`trigger_zone`/`npc` any more. Verify with
   `rg 'Interactable\s*\{'` — src should show exactly 2 hits (the struct def + entity_spawner).
4. Pure-lookup helper `capabilities::inventory::has_item(slots, key) -> bool` (count-agnostic;
   safe because `remove_from_slots` nulls a slot at `count == 0`).
5. System reads `Res<PlayerInventory>` and branches which `GameEvent::Trigger` string it writes —
   **never touches `ActionQueue`**. This is the correct capability posture.
6. CLI `validate` reference check (prefab-catalog-scoped, sorted keys, `missing_reference`,
   `source_file: "prefabs/prefabs.ron"`) + `bad_`/`valid_` fixture pair.
7. Scene-load `warn!` twin (`scene_loader.rs::warn_missing_interactable_item_key`, called from
   the `warn_*` block at ~line 814-820 alongside `warn_missing_stat_widget_templates`).
8. Shipped example in `3rd_person_game_demo` (prefab + scene entity + `global_on:` pair).

**The `hit_any` polarity is the load-bearing subtlety.** A blocked interact must still set
`hit_any = true`, or `player.attack_missed` fires alongside `entity.interact_blocked`. The plan's
first draft had this backwards. Any future "refuse the interact" condition (locked-by-quest,
locked-by-stat) must keep the same polarity — the press *landed*, it was refused.

**Two scoping asymmetries a copy of this pattern inherits (both non-blocking, both worth naming):**
- The CLI check is **catalog-wide** (every prefab) but the runtime `warn!` is **scene-scoped**
  (`scene.entities` only). So a gated prefab reachable only through `Action::Spawn` gets no runtime
  signal — which undercuts the "WASM-only designer with no CLI" justification the warn exists for.
- The runtime warn early-returns on `item_catalog == None`. That is **correct, not a hole**:
  `add_to_slots` accepts any `item_key` with no catalog entry, so a catalog-less project can
  legitimately grant `"magic_key"` via `AddItem` and gate on it. Do not recommend warning there.

**`PlayerInventory` is deliberately global/single, not per-player.** `interactable_system` loops
per-player (each player's own interact key) but resolves the gate against the one shared
`PlayerInventory` resource — so in local co-op any one player's key unlocks for everyone. This is
the same scope every other inventory action already has (`AddItem`/`RemoveItem`/`BuyItem` route the
literal `"player"` id to this resource). **Accept it as an established boundary; do not flag it as a
hardcoding violation.** Contrast [[per-player-stat-pools-pattern]], where the per-player split
*was* built — inventory has no equivalent and none is needed for this feature.

**No wildcard on `entity.*` events** — `entity.interact_blocked:{id}` is exact-match only, same as
`entity.interacted:{id}`. The scalable authoring answer for N gated doors sharing one prefab is a
`.behavior.ron` with `event: "entity.interact_blocked:{self}"` (behavior FSMs substitute `{self}`
in *event patterns*, not just action fields). Neither the docs nor the shipped example show this
form — recommend it whenever a per-entity `entity.*` event gets a new sibling.

**`dialogue.rs:105` uses `strip_prefix("entity.interacted:")`** — `entity.interact_blocked:` does
not collide with it, so item-gating a `dialogue:`-carrying prefab correctly gates the conversation
too (a deliberate, documented free side effect, not an accident). Check this prefix-collision
whenever adding an `entity.interact*` sibling.

**Recurring miss confirmed again:** `docs/60_contributing.md`'s "Checks performed" list
(~lines 236-265) was NOT updated for the new validate check — see
[[validate-cross-file-blind-spots]]; this is now ~5 consecutive validate.rs features with the same
omission. Also missed here: `crates/ironhold_core/src/CLAUDE.md`'s `Interactable` "Emits:" bullet
list (~line 429).
