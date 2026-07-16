# Feature: Action Bar Custom Hotkeys

_Status: Done_
_Planned at: `a6acab8` (2026-06-22)_
_Plan review (2026-07-15): system-architect + ux-gamedesigner-reviewer, verdict Needs-more-design-
work on both passes. system-architect caught a real, shippable regression: the Migration
section's claim that `parse_key("i")` "accepts the lowercase letter form" is false —
`parse_key` is case-sensitive with uppercase-only letter arms, so `3rd_person_game_demo`'s
existing `key: "i"` inventory slot would go silently dead under this change, exactly the bug the
feature exists to fix. ux-gamedesigner-reviewer caught that the plan never accounts for the
already-shipped `ActionSlotDef.label` field, creating a `label`/`key_hint` naming collision, and
that unparseable key names and duplicate-key collisions both need a load-time `warn!` (not just an
opt-in CLI `validate()` a designer never runs). All incorporated below — see Approach, Tasks,
Decisions, and the new "Relationship to Phase 2" section. Reaches Ready._
_Code review (2026-07-16): alignment-reviewer (ALIGNED), system-architect (no blockers; 2 minor —
a misleading CLI comment fixed, cross-bar duplicate-key gap logged to `claude_suggestions.md`),
debug-detective (essentially clean; same cross-bar gap independently flagged, converged with
system-architect's finding; a slightly overstated code comment fixed), ux-gamedesigner-reviewer
(no blockers; `label` field silently rendered nowhere — added a "not yet displayed" doc caveat and
reconciled the demo slot's `label`/`key_hint` strings so they agree; added modifier/arrow-key
corner-glyph guidance and numpad/digit-0 to the accepted-key-names list), wasm-perf-reviewer (OK,
neutral-to-positive frame-time impact, no concerns). All findings addressed or logged. Full
`ironhold_core` test suite (16 binaries) + `ironhold_cli` test suite green. WASM dev build clean.
Playtest confirmed by Frank in `3rd_person_game_demo` — existing `1`-`9`/`i` slots unaffected, new
`KeyE` "Taunt" slot works (including monster aggro, added after Frank's playtest feedback that the
demo slot should actually deal damage). No console warnings about keys — `8df3cfc`._

## What
Make action-bar slot hotkeys fully designer-configurable from RON. Today a slot can only be
activated by the keys `1`–`9` or `i`, because `action_bar_input_system` looks the pressed key up
against a hardcoded `DIGIT_KEYS` table in `capabilities/action_bar.rs`. After this change, a
designer can bind any slot to any key name that `InputMap::parse_key()` understands
(letters, digits, `F1`–`F12`, modifiers, arrows, `Space`, `Escape`, `Tab`, `Enter`,
`Backspace`, `Delete`) directly in the scene RON's `ActionSlotDef`, e.g. `key: "KeyQ"`,
`key: "KeyE"`, `key: "F2"`. No engine recompile, no expansion of a hardcoded table.

## Why
This is a straight data-driven-integrity fix. The schema already advertises a designer-authored
`ActionSlotDef.key` field, and `InputMap::parse_key()` already provides the full
string→`KeyCode` mapping used everywhere else for key binding (`global_key_bindings`,
`scene_key_bindings`, player `InputMap`). The only thing standing between the two is the
hardcoded `DIGIT_KEYS` lookup. Right now a designer who writes `key: "q"` gets a silently dead
slot — the worst kind of data-driven failure, because the RON parses fine and the button renders,
but the key never fires. Closing this gap removes a hardcoded behavior bottleneck and makes the
action bar consistent with the rest of the input-binding surface. It unblocks ability layouts
beyond the numeric row (QWER-style action games, F-key utility slots) without touching Rust.

## Approach

### Root cause (verified)
`capabilities/action_bar.rs` lines 202–214 define `DIGIT_KEYS: &[(KeyCode, &str)]`.
`action_bar_input_system` finds the first `just_pressed` entry in that table, takes its string
(`"1"`..`"9"`, `"i"`), and matches it against `ActionSlotUi.slot_key`. So the *only* keys that
can ever activate a slot are the ten hardcoded ones, and matching is string-equality on the raw
`key` field — `key: "q"` in RON parses but matches nothing.

### Key insight: `slot.key` currently serves THREE purposes (verified in scene_loader.rs ~L1720–1844)
The same `ActionSlotDef.key` string is reused as:
1. **Slot identity** — `ActionSlotUi.slot_key`, used as the `CooldownMap`/`CooldownOverlay` key and
   embedded in every emitted event (`action_bar.activated:{key}`, `action_bar.on_cooldown:{key}`,
   `action_bar.insufficient_resource:{key}`, `action_bar.no_target:{key}`).
2. **Key binding** — the value `action_bar_input_system` matches against the pressed key.
3. **On-screen hint label** — the scene loader renders `Text::new(key.clone())` in the slot's
   bottom-right corner (scene_loader.rs ~L1841).

Because of (1), the chosen design must NOT silently change what `slot_key` contains, or it would
break existing `state_machine.ron` / `rules.ron` that listen for `action_bar.activated:1`,
cooldown tracking keyed on `"1"`, etc.

### Recommended design: keep `key` as identity+binding, add optional `key_hint` for the corner glyph

The cleanest minimal-footprint change keeps `ActionSlotDef.key` exactly as it is — it remains both
the slot identity and the binding — and only changes how the runtime *interprets* it:

- **Runtime change (the actual fix):** in `action_bar_input_system`, replace the `DIGIT_KEYS`
  scan with a per-slot `InputMap::parse_key(&slot.slot_key)` resolution. Iterate the slots, parse
  each slot's key string to a `KeyCode`, and check `keys.just_pressed(kc)`. This makes any
  `parse_key`-recognised string live, and it deletes the `DIGIT_KEYS` constant entirely.
- **Slot identity:** unchanged. `slot_key` stays the literal RON string (`"1"`, `"KeyQ"`, `"F2"`).
  All `action_bar.*` events and cooldown keys continue to use it verbatim. No event/cooldown
  consumer needs to change. **Note the flip side of this (ux-gamedesigner-reviewer):** because
  `key` is simultaneously the binding, the cooldown key, and the event name, a designer who
  *rebinds* a slot (e.g. `"1"` → `"KeyQ"`) silently also renames its entire event contract
  (`action_bar.activated:1` → `action_bar.activated:KeyQ`) and cooldown identity, breaking any
  `rules.ron`/`state_machine.ron` wired to the old strings. Accepted tradeoff (same one this
  section already weighs against the rejected `hotkey` split below) — must be called out explicitly
  in docs, not left for a designer to discover.
- **New field is `key_hint`, distinct from the existing `ActionSlotDef.label` field — do not
  confuse the two (ux-gamedesigner-reviewer, headline finding: the original draft used `key_label`,
  which collides visually and semantically with the pre-existing `label` field already used by
  every shipped action bar).** `label` is the slot's ability/tooltip name (e.g. `"Heavy Strike"`,
  already documented separately); the new `key_hint: Option<String>` (default `None`) is purely the
  on-screen key glyph in the button's corner. The raw `key` string is a poor glyph for non-digit
  keys — `"KeyQ"` would render literally as "KeyQ". The scene loader renders `key_hint` when set,
  otherwise falls back to a short pretty-print of `key` (strip the `Key` prefix so `"KeyQ"` → `"Q"`,
  leave digits and `F2` as-is). This keeps existing digit bars rendering identically while giving
  designers control over the corner glyph for letter/F-key slots. The RON example and the
  `docs/20_data_formats.md` field table must show **both** `label` and `key_hint` set on the same
  slot so the distinction is unambiguous, and `label` (currently missing from that table entirely,
  a pre-existing gap) must be added alongside it.
- **Unparseable key names must not silently produce a dead slot (ux-gamedesigner-reviewer, second
  headline finding).** The entire point of this feature is closing "RON parses fine, button
  renders, key never fires." Simply forwarding an unrecognised `key` string to `parse_key()` (which
  returns `None`) reintroduces that exact bug for typos (`"KeyQQ"`) or plausible-but-unsupported
  names (`"MouseLeft"`, `"Shift+1"`, a casing form `parse_key` doesn't accept). Add: (a) a
  scene-load-time `warn!` naming the slot and the unparseable key string, in addition to (b) a
  `validate()` error in the CLI tool. Both signals matter — see the duplicate-key decision below for
  why `validate()` alone isn't sufficient for this audience.

### Rejected alternative: separate `hotkey` field
Splitting binding into a new `hotkey` field while `key` stays identity was considered. It is more
explicit but heavier: it adds a second required-ish field, creates a "which one do I set?" authoring
ambiguity, and offers no benefit unless we want two slots to share an identity but differ in
binding (no use case today). Defer unless a concrete need appears. If adopted later it is an
additive, backward-compatible change (`hotkey` defaults to `key`).

### RON example (after this feature)
```ron
ActionBar((
    id: "skills",
    position: (400.0, 640.0),
    icon_sheet: "icons_abilities",
    slots: [
        // Numeric slot — unchanged, renders "1", fires on Digit1.
        (key: "1", icon_index: 0, label: "Swing", do_actions: [PlaySound(key: "swing")]),

        // QWER layout — bind slot to the Q key; hint shows "Q" automatically.
        // `label` (tooltip/ability name) and `key_hint` (corner glyph) are independent fields —
        // shown together here so the distinction is unambiguous.
        (key: "KeyQ", icon_index: 1, cooldown_secs: 2.0, label: "Heavy Strike",
         do_actions: [PlaySound(key: "swing"), ModifyStat(key: "{target}.health", delta: -30.0)]),

        // Bind to E; override the on-screen corner glyph with a custom word (label stays separate).
        (key: "KeyE", icon_index: 2, label: "Dodge Roll", key_hint: "Dash",
         do_actions: [PlayAnimationOn(target: "player", clip: "roll")]),

        // Utility slot on a function key.
        (key: "F2", icon_index: 3, label: "Inventory",
         do_actions: [OpenInventory]),
    ],
))
```
Cooldown tracking and events for the QWER slot key off the literal strings:
`action_bar.activated:KeyQ`, `action_bar.on_cooldown:KeyQ`, etc. Designers wire
`state_machine.ron` / `rules.ron` against those exact strings.

### Accepted key-name forms (must be documented explicitly, not left as "whatever `parse_key`
accepts" — ux-gamedesigner-reviewer: a designer can't read the Rust source)
`docs/20_data_formats.md` must list the concrete accepted forms for common keys designers will
reach for first (bare letters `"q"`/`"Q"`, `"KeyQ"`, digits `"1"`-`"9"`, `"F1"`-`"F12"`, `"Space"`,
`"Escape"`, `"Tab"`, `"Enter"`, `"Backspace"`, `"Delete"`, arrow keys) and their exact corner-hint
pretty-print result for each form — plus an explicit **"not supported" list**: mouse buttons,
modifier chords (`"Shift+1"`), and gamepad buttons all silently fail to bind today (see the
unparseable-key handling above) and are out of scope for this feature (see Out of scope).

## Tasks
- [x] **Fix `InputMap::parse_key()` to accept lowercase single letters (must land before the
      `DIGIT_KEYS` removal below — see Migration).** `parse_key` (`schema/player.rs:248`) is
      case-sensitive with uppercase-only letter arms (`"KeyI" | "I" => ...`) and `_ => None` —
      `parse_key("i")` returns `None` today. Add lowercase arms (or a `.to_ascii_uppercase()`
      normalization pass) so `"i"` keeps resolving to `KeyCode::KeyI`. This is a real code task,
      not a docs note (system-architect, Critical finding).
- [x] Remove `DIGIT_KEYS` from `capabilities/action_bar.rs`.
- [x] Rewrite `action_bar_input_system` to resolve each slot's `slot_key` via
      `InputMap::parse_key()` and check `just_pressed` on the resulting `KeyCode`.
      Keep the existing cooldown / cost / `{target}` / fire logic unchanged; only the
      key-detection front of the system changes.
- [x] Handle unparseable key names: when `parse_key(&slot.slot_key)` returns `None`, emit a
      scene-load-time `warn!` naming the slot and the bad key string, and add a matching
      `validate()` error in the CLI tool (ux-gamedesigner-reviewer — without the runtime `warn!`,
      this reintroduces the exact silent-dead-slot bug the feature exists to close, since a
      designer testing in the browser never sees CLI output).
- [x] Handle the multi-slot edge case: if two slots resolve to the same `KeyCode`, the system
      currently fires the first match. **Duplicate-key warning must be a scene-load-time `warn!`,
      not `validate()`-only** (ux-gamedesigner-reviewer — same reasoning as the unparseable-key
      task above), in addition to a `validate()` check in the CLI tool. See Decisions.
- [x] Add `key_hint: Option<String>` to `ActionSlotDef` (`#[serde(default)]`) — **distinct from the
      pre-existing `label` field** (ux-gamedesigner-reviewer headline finding: an earlier draft
      named this `key_label`, which collides with `label`). `label` is the ability/tooltip name;
      `key_hint` is only the corner key-glyph override.
- [x] Scene loader: render `key_hint` when set; otherwise pretty-print `key` (strip `Key` prefix).
- [x] Update the `ActionSlotDef.key` doc comment (currently says "`\"1\"` through `\"9\"`") and the
      `ActionBarDef` doc comment (currently says "bound to keys 1–9") to state any `parse_key` name
      is accepted.
- [x] Add `label` to the `docs/20_data_formats.md` `ActionSlotDef` field table (currently missing
      entirely, a pre-existing gap this feature is already editing that table for).
- [x] Add a working non-digit example slot (e.g. `key: "KeyQ"`) to a shipped project
      (`3rd_person_game_demo`'s action bar) so there's a real, browsable reference — not just a
      docs-only example (ux-gamedesigner-reviewer).
- [x] CLI: `cargo check -p ironhold_cli` (new optional field must not break `query.rs`).
- [x] Tests: extend the action-bar integration tests to cover a letter-key slot
      (`key: "KeyQ"`) firing on `KeyCode::KeyQ`, an `F2` slot firing on `KeyCode::F2`, and a
      regression test that `3rd_person_game_demo`'s existing `key: "i"` inventory slot still fires
      after the `parse_key` fix above (system-architect: this is the one real migration case, not
      hypothetical — must be covered, not just asserted). Add a `ron_validation`/`ron_lint` case for
      both the duplicate-key and unparseable-key `validate()` checks.
- [x] Docs: `docs/20_data_formats.md` (ActionSlotDef field table incl. `label` + `key_hint`,
      accepted-key-name-forms reference, "not supported" list — see the new section above),
      `docs/30_runtime_events_and_logic.md` if action-bar events are documented there, and the
      action-bar notes in `crates/ironhold_core/src/CLAUDE.md` if present. Include the
      slot-rebinding-changes-event-names caveat (see Recommended design) and note the action bar
      remains keyboard-only (a gamepad player's slots never fire, regardless of key name).

## Relationship to Phase 2 (`per_player_split_screen_targeting.md`)
This feature is a hard dependency of that plan's Phase 2 (per-player action-bar execution) — two
split-screen players need disjoint, non-digit-only hotkeys to avoid colliding on the same physical
key. Sequencing: this feature ships first (its own branch, merged to `main`), then Phase 2 branches
off the updated `main`. Two coordination notes for whoever implements Phase 2 afterward:
- **Don't over-invest in this feature's fire-first duplicate-key handling** — Phase 2 immediately
  restructures `action_bar_input_system`'s `find`+`return` single-match structure into a loop over
  every `just_pressed` slot (required there because two players' simultaneous presses must both
  register, not just one). This feature's own duplicate-key/unparseable-key `warn!`s should be
  written so they still make sense against that later loop structure (i.e., check every slot, not
  just the first match found).
- **Design the duplicate-key `validate()`/`warn!` check to be cross-bar-extensible** — this feature
  only needs to check within one bar (today there's exactly one bar per scene), but Phase 2
  introduces multiple bars per scene and needs the same check extended across all of them. Structure
  the check as "collect all resolved keys scene-wide, report collisions" rather than "check within
  this one bar's `slots` list", so Phase 2 can reuse it directly instead of duplicating the logic.

## Migration
**Existing scene RON keeps working, contingent on the `parse_key` lowercase-letter fix above.** All
current action bars use `key: "1"`..`"9"` and `key: "i"`. `InputMap::parse_key("1")` returns
`KeyCode::Digit1` — digits are unaffected either way. **`parse_key("i")` does NOT resolve today**
(system-architect, verified against `schema/player.rs:248`: letter arms are uppercase-only,
`_ => None`) — without the lowercase-letter fix task above, `3rd_person_game_demo`'s existing
`key: "i"` inventory slot would go silently dead the moment `DIGIT_KEYS` is removed. With that fix
landed first, `parse_key("i")` resolves to `KeyCode::KeyI` and every existing binding matches the
old `DIGIT_KEYS` table's output. `slot_key` strings are untouched, so all existing
`action_bar.activated:1` etc. event wiring and cooldown tracking continue to match.
Verified projects with action bars: `3rd_person_game_demo` (includes the one `key: "i"` slot —
`scenes/main.scene.ron`), `primitive_world`, `stats_demo` — all others use numeric `key: "1".."9"`
and need no RON edits. The on-screen hint also renders identically: a digit key pretty-prints to
itself. (The plan's own field-of-view was previously incomplete here — see the `label` field note
in Recommended design; the migration claim above concerns only `key`/hint rendering, not `label`,
which is untouched by this feature either way.)

**Add the `"i"` regression test to Tasks** (done above) — do not merely assert this case works.

## Relationship to "Input remapping" (Icebox)
This is **designer-time** configuration: the binding is authored in scene RON and fixed at load.
It is NOT the Icebox "Input remapping" feature, which is **runtime player rebinding** via a
settings UI with persistence to config / localStorage and gamepad support. The two are
complementary: this feature decides the *default* binding a designer ships; the future remapping
feature would let a *player* override it at runtime. When remapping eventually lands, action-bar
slot keys should flow through the same binding-resolution layer rather than reading `slot.key`
directly — note that as a forward dependency, but do not build it now.

## Out of scope
- Runtime player rebinding / settings UI / persistence.
- Gamepad button bindings.
- Mouse-button activation of slots.
- Modifier-chord bindings (e.g. `Shift+1`) — `parse_key` returns a single `KeyCode`; chords would
  need a new parse path and are not requested.
- Reworking `slot_key` into a separate identity vs. binding split (the rejected `hotkey`
  alternative) — deferred until a concrete need exists.

## Decisions
- **Duplicate resolved keys within one bar:** fire-first + a **scene-load-time `warn!`** (not
  `validate()`-only, revised per ux-gamedesigner-reviewer) plus a `validate()` check in the CLI
  tool. Not a hard error — it is a footgun, not a correctness bug, but it must be visible to a
  designer testing in the browser, not just to someone who remembers to run the CLI.
- **Unparseable key names:** scene-load-time `warn!` + `validate()` error (revised per
  ux-gamedesigner-reviewer — see Tasks). A slot with a key `parse_key` can't resolve never fires;
  silently doing nothing is exactly the bug this feature exists to close.
- **Hint pretty-printing:** strip `Key` prefix only (`"KeyQ"` → `"Q"`). `key_hint` covers anything
  fancier. No canonical display map in v1.
- **`key_hint` vs. `label`:** kept as two separate fields (not merged, not renamed to overlap) —
  `label` is the ability/tooltip name, `key_hint` is only the corner key-glyph override. See
  Recommended design.

## Acceptance criteria
- Given a scene with `(key: "KeyQ", do_actions: [...])`, when the player presses `Q`, then the
  slot's `do_actions` fire, the cooldown starts, and `action_bar.activated:KeyQ` is emitted.
- Given a scene with `(key: "F2", do_actions: [OpenInventory])`, when the player presses `F2`,
  then the inventory opens.
- Given any existing project using `key: "1"`..`"9"`, when run after the change, then behavior,
  events, cooldowns, and on-screen hints are identical to before.
- Given `3rd_person_game_demo`'s existing `key: "i"` inventory slot, when run after the change
  (with the `parse_key` lowercase-letter fix landed), then it still fires on the `I` key — this is
  the one real migration case and must be covered by a test, not just asserted (system-architect).
- Given `(key: "KeyE", key_hint: "Dash")`, then the slot's corner hint renders "Dash"; with no
  `key_hint`, a `"KeyE"` slot renders "E"; the slot's separate `label` field (if set) is unaffected
  and renders wherever `label` already renders today.
- Given a slot with an unparseable key (e.g. `key: "MouseLeft"`), when the scene loads, then a
  `warn!` fires naming the slot and the bad key string, and `ironhold_cli validate` reports it too.
- Given two slots in one bar that resolve to the same `KeyCode`, when the scene loads, then a
  `warn!` fires identifying the collision, and the first-matching slot fires on press (documented,
  not silent).
