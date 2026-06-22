# Feature: Action Bar Custom Hotkeys

_Status: Draft_
_Planned at: `a6acab8` (2026-06-22)_

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

### Recommended design: keep `key` as identity+binding, add optional `label` override for the hint

The cleanest minimal-footprint change keeps `ActionSlotDef.key` exactly as it is — it remains both
the slot identity and the binding — and only changes how the runtime *interprets* it:

- **Runtime change (the actual fix):** in `action_bar_input_system`, replace the `DIGIT_KEYS`
  scan with a per-slot `InputMap::parse_key(&slot.slot_key)` resolution. Iterate the slots, parse
  each slot's key string to a `KeyCode`, and check `keys.just_pressed(kc)`. This makes any
  `parse_key`-recognised string live, and it deletes the `DIGIT_KEYS` constant entirely.
- **Slot identity:** unchanged. `slot_key` stays the literal RON string (`"1"`, `"KeyQ"`, `"F2"`).
  All `action_bar.*` events and cooldown keys continue to use it verbatim. No event/cooldown
  consumer needs to change.
- **UI hint label:** the raw `key` string is a poor hint for non-digit keys — `"KeyQ"` would render
  literally as "KeyQ" in the corner of the button. Add an optional `key_label: Option<String>` to
  `ActionSlotDef` (default `None`). The scene loader renders `key_label` when set, otherwise falls
  back to a short pretty-print of `key` (strip the `Key` prefix so `"KeyQ"` → `"Q"`, leave digits
  and `F2` as-is). This keeps existing digit bars rendering identically while giving designers
  control over the hint text for letter/F-key slots.

This design adds exactly one optional schema field, one runtime-system rewrite, and one
scene-loader label tweak. `slot_key` semantics are preserved, so it is fully backward compatible.

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
        (key: "1", icon_index: 0, do_actions: [PlaySound(key: "swing")]),

        // QWER layout — bind slot to the Q key; hint shows "Q" automatically.
        (key: "KeyQ", icon_index: 1, cooldown_secs: 2.0,
         do_actions: [PlaySound(key: "swing"), ModifyStat(key: "{target}.health", delta: -30.0)]),

        // Bind to E; override the on-screen hint with a custom glyph/word.
        (key: "KeyE", icon_index: 2, key_label: "Dash",
         do_actions: [PlayAnimationOn(target: "player", clip: "roll")]),

        // Utility slot on a function key.
        (key: "F2", icon_index: 3,
         do_actions: [OpenInventory]),
    ],
))
```
Cooldown tracking and events for the QWER slot key off the literal strings:
`action_bar.activated:KeyQ`, `action_bar.on_cooldown:KeyQ`, etc. Designers wire
`state_machine.ron` / `rules.ron` against those exact strings.

## Tasks
- [ ] Remove `DIGIT_KEYS` from `capabilities/action_bar.rs`.
- [ ] Rewrite `action_bar_input_system` to resolve each slot's `slot_key` via
      `InputMap::parse_key()` and check `just_pressed` on the resulting `KeyCode`.
      Keep the existing cooldown / cost / `{target}` / fire logic unchanged; only the
      key-detection front of the system changes.
- [ ] Handle the multi-slot edge case: if two slots resolve to the same `KeyCode`, the system
      currently fires the first match. Decide and document (fire first, or warn at load). Recommend
      fire-first + a `validate()` warning on duplicate resolved keys within one bar.
- [ ] Add `key_label: Option<String>` to `ActionSlotDef` (`#[serde(default)]`).
- [ ] Scene loader: render `key_label` when set; otherwise pretty-print `key` (strip `Key` prefix).
- [ ] Update the `ActionSlotDef.key` doc comment (currently says "`\"1\"` through `\"9\"`") and the
      `ActionBarDef` doc comment (currently says "bound to keys 1–9") to state any `parse_key` name
      is accepted.
- [ ] CLI: `cargo check -p ironhold_cli` (new optional field must not break `query.rs`).
- [ ] Tests: extend the action-bar integration tests to cover a letter-key slot
      (`key: "KeyQ"`) firing on `KeyCode::KeyQ`, and an `F2` slot firing on `KeyCode::F2`.
      Add a `ron_validation` / `ron_lint` case if duplicate-resolved-key warning is added.
- [ ] Docs: `docs/20_data_formats.md` (ActionSlotDef field table + key-name reference),
      `docs/30_runtime_events_and_logic.md` if action-bar events are documented there, and the
      action-bar notes in `crates/ironhold_core/src/CLAUDE.md` if present.

## Migration
**Existing scene RON keeps working unchanged.** All current action bars use `key: "1"`..`"9"`
and `key: "i"`. `InputMap::parse_key("1")` returns `KeyCode::Digit1`, `parse_key("i")` returns
`KeyCode::KeyI` (it accepts the lowercase letter form) — so every existing binding resolves to the
same `KeyCode` the old `DIGIT_KEYS` table produced. `slot_key` strings are untouched, so all
existing `action_bar.activated:1` etc. event wiring and cooldown tracking continue to match.
Verified projects with action bars: `3rd_person_game_demo`, `primitive_world`, `stats_demo` — all
use numeric `key: "1"..` and need no edits. The on-screen hint also renders identically: a digit
key pretty-prints to itself.

One behavioral caveat worth noting in the changelog: today, the lowercase `"i"` slot only works
because `"i"` happens to be in `DIGIT_KEYS`. After the change, the lowercase form still resolves
(`parse_key` accepts `"i"`), so no migration is needed — but designers should be encouraged toward
the canonical `"KeyI"` form for clarity in new content.

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
- **Duplicate resolved keys within one bar:** fire-first + `validate()` warning. Not a hard error —
  it is a footgun, not a correctness bug.
- **Hint pretty-printing:** strip `Key` prefix only (`"KeyQ"` → `"Q"`). `key_label` covers anything
  fancier. No canonical display map in v1.

## Acceptance criteria
- Given a scene with `(key: "KeyQ", do_actions: [...])`, when the player presses `Q`, then the
  slot's `do_actions` fire, the cooldown starts, and `action_bar.activated:KeyQ` is emitted.
- Given a scene with `(key: "F2", do_actions: [OpenInventory])`, when the player presses `F2`,
  then the inventory opens.
- Given any existing project using `key: "1"`..`"9"`, when run after the change, then behavior,
  events, cooldowns, and on-screen hints are identical to before.
- Given `(key: "KeyE", key_label: "Dash")`, then the slot's corner hint renders "Dash"; with no
  `key_label`, a `"KeyE"` slot renders "E".
