# runtime/ — boot and update systems outside `scene_manager/`

Rules for `input.rs`, `actions.rs`, `messages.rs`, `model_spawner.rs`, `material_factory.rs` and `mod.rs`. Loaded
when you touch a file in this folder (`scene_manager/` has its own `CLAUDE.md`); the crate-wide rules are in
`../CLAUDE.md`.

## Topic references

- Gamepad binding, claiming and hot join, in full: `docs/dev/gamepad-routing.md` (the two rules in this file are the
  part that must never be violated).

<!-- b:1530.rule -->
## Gamepad binding (`input.rs`)

`InputMap.gamepad_index` is only a **one-time seed**. The source of truth every gamepad consumer reads is
`BoundGamepad(pub Option<Entity>)` (`capabilities/player.rs`): `None` is "pending", `Some(entity)` is locked to that
gamepad entity for the player's lifetime. Gamepad and keyboard are read additively (`||`), never exclusively.

- **`gamepad_bind_system` (`FixedUpdate`, `.before(input_translator_system)`) must never bind a player to an `Entity`
  another player already holds, even across frames.** It keeps a `claimed: HashSet<Entity>` seeded from every
  already-bound player **and from every undrained `is_hot_join` spawn's captured `bound_gamepad`** (a hot-joined
  player can wait in `PendingEntitySpawns` for a frame or more), grown as the pass binds new ones. Without the
  `claimed` check a pad that connects later with a lower `Entity::index()` shifts the sorted list and a pending
  player's seed resolves to a pad another player already owns.
- Pending players are visited in **ascending `PlayerIndex` order**, so which player wins a duplicated seed is
  deterministic, not an accident of query order. A displaced player stays pending (no auto-rebind this session) and
  gets one `warn!` after `GAMEPAD_DIAGNOSTIC_WARN_SECS` (3.0).
- `unclaimed_gamepad_trigger_system` (`Update`) must also reserve the pad a still-pending live player's seed is about
  to resolve to: `FixedUpdate` can tick zero times in a frame, so on the frame a pad first appears
  `gamepad_bind_system` may not have run and the pad would look unclaimed for one frame.

Consumers of the pad (`input_translator_system` here, and the gamepad readers in `capabilities/`) take `Option<&BoundGamepad>`,
**never a required `&BoundGamepad`** (a required one silently drops any test-built player missing it from the whole query tuple,
not just gamepad logic), and look the pad up with `bound.and_then(|b| b.0).and_then(|e| gamepad_query.get(e).ok())`.

<!-- b:1606.rule -->
## Gamepad-triggered hot join (`input.rs`)

`unclaimed_gamepad_trigger_system` (`.before(fsm_interpreter_system)`) checks the global unclaimed-gamepad bindings
only against pads **not** claimed by a live player's `BoundGamepad`, by an undrained `is_hot_join` entry's captured
`bound_gamepad`, or by a pending player's seed resolving this frame. On a `just_pressed` match it emits the usual
`UiEvent::ButtonPressed` and writes the pad's `Entity` into `PendingJoinGamepad(Option<Entity>)`.

- **At most one pad is captured per frame** (lowest `Entity::index()` wins), and the resource is **reset to `None`
  unconditionally at the top of every run**: otherwise a non-join gamepad trigger (a pause button) leaves a stale pad
  identity for a later keyboard-triggered join to inherit.
- `Action::JoinPlayer`'s executor arm `.take()`s `PendingJoinGamepad` and writes it straight into
  `PlayerConfig.bound_gamepad`, with no round trip through `inputs.gamepad_index`: re-resolving an index to an
  `Entity` a frame later is a window in which pad churn binds the wrong device. A keyboard-triggered join sees
  `None` and is unaffected.
