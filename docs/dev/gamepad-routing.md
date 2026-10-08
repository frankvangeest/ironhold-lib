# Gamepad routing: binding, claiming and hot join

`BoundGamepad`, the `claimed` invariant and the gamepad-triggered hot join path.

> Moved from `crates/ironhold_core/src/CLAUDE.md` on 2026-10-08 (the core CLAUDE.md split). The hard rules for this area stay in the folder `CLAUDE.md` files; this page keeps the long-form reference and history verbatim.

## Gamepad routing: `BoundGamepad`, `claimed` invariant

<!-- b:1530.rule -->
<!-- b:1530.ref -->

**Gamepad routing (post-`gamepad_player_binding_hardening.md`)** — `InputMap.gamepad_index:
Option<usize>` lets a player prefab bind to a specific gamepad *in addition to* the keyboard
(additive, not a replacement — see the doc comment on the field itself), but it is only ever read
as a **one-time seed**, never a live positional lookup. `BoundGamepad(pub Option<Entity>)`
(`capabilities/player.rs`) is the actual source of truth every gamepad-consuming system reads:
`None` = "pending" (no seed authored, or the seed hasn't resolved to a live pad yet); `Some(entity)`
= "bound" — locked to that specific gamepad `Entity` for the player's whole lifetime (barring a
future hot-leave/rejoin). `gamepad_bind_system` (`runtime/input.rs`, `FixedUpdate`,
`.before(input_translator_system)`) is the system that *enforces* the invariant described here — the only
other writer is the hot-join spawn site, which seeds `BoundGamepad` directly from an
already-known-good `Entity` at construction time (see "Gamepad-triggered hot join" later on this page), not by
resolving a seed. `gamepad_bind_system` visits every player in one pass each tick (in ascending
`PlayerIndex` order for the pending ones, so which player wins a duplicated seed is deterministic,
not an accident of archetype/query order) and, for each still-pending player, attempts
`sorted_gamepads.get(seed)` (sorted by `Entity::index()`, built fresh each call) against a
`claimed: HashSet<Entity>` seeded from every already-bound player **and every undrained
`is_hot_join` spawn's own captured `bound_gamepad`** (a hot-joined player can sit in
`PendingEntitySpawns` for a frame or more before `drain_spawn_queue_system`'s rate limit lets it
through — without this half, a pending scene player could bind to the same pad in that window;
system-architect/debug-detective finding, post-implementation review) and grown as the same pass
binds new ones — a **hard invariant**: it will never bind a player to an `Entity` any other player
already holds, even across frames (the cross-time race this exists to close: pad B connects first
and binds to P1's seed 0; P2's seed 1 is out of range, stays pending; pad A connects later with a
*lower* `Entity::index()` than B, so the sorted slice becomes `[A, B]` — without the `claimed` check
P2's seed 1 would now resolve to B, already bound to P1). A displaced pending player just stays
pending — no auto-rebind to a different, now-free pad this session — and gets a one-shot `warn!`
(`GAMEPAD_DIAGNOSTIC_WARN_SECS = 3.0`) if the stuck state persists, same mechanism used to diagnose
a *bound* player whose `Gamepad` component has disappeared (disconnected); both timer `Local`s are
pruned each call against the live player set, so a player who despawns while stuck doesn't leak an
entry forever. `unclaimed_gamepad_trigger_system` (`Update`) additionally reserves the pad a
still-pending *live* player's seed is about to resolve to — needed because `FixedUpdate`'s
accumulator can tick zero times in a frame, so on the exact frame a pad first becomes visible (its
first press, also this system's join-trigger frame on the web) `gamepad_bind_system` may not have
run yet and the pad would otherwise look unclaimed for one frame (debug-detective finding). Once
bound, the four simple consumers (`input_translator_system`, `tab_targeting_system`,
`interactable_system`, `action_bar_input_system`) take `Option<&BoundGamepad>` (not required —
a required `&BoundGamepad` would silently drop any test-constructed player entity missing it out
of the *entire* query tuple, not just gamepad logic) and do a direct
`bound.and_then(|b| b.0).and_then(|e| gamepad_query.get(e).ok())` — no sorting, no re-deriving
position, immune to any other pad's connect/disconnect churn. `camera_orbit_system` has no player
`Entity` in its own query, so it resolves via a disjoint `bound_q: Query<&BoundGamepad>` looked up
through `CameraTargets`; a spawn-frozen positional `gamepad_index` copy on the camera itself was
never introduced, precisely to avoid it silently diverging from `BoundGamepad`. The old crate-shared
`resolve_gamepad` helper was removed — nothing needs a live
positional lookup anymore.

## Gamepad button/axis RON mapping, parity gap

<!-- b:1576 -->

Button/axis mapping is fully RON-configurable via `InputMap` — `gamepad_jump`/`gamepad_run`/
`gamepad_interact`/`gamepad_target_next: String` (parsed via `InputMap::parse_gamepad_button`,
mirroring `parse_key`'s validation seam — an unrecognized name `warn!`s and no-ops rather than
crashing) and `gamepad_deadzone: f32`, all defaulting to the same values every scene had hardcoded
before this field existed (`South`/`East`/`West`/`North`/`0.15`). Left stick moves/strafes, right
stick X turns, right stick Y drives camera pitch — independent of the keyboard's
`strafe_mouse_button` toggle (that only exists to disambiguate A/D on one keyboard; a gamepad
already has separate sticks). `gamepad_interact`/`gamepad_target_next` fold into
`interactable_system`'s/`tab_targeting_system`'s existing per-player `keyboard || gamepad`
boolean, so both work in local co-op, not just single-player — no gamepad path exists for camera
*yaw* (right-stick-X already drives character turning), a permanent, deliberate keyboard/gamepad
parity gap, not an oversight (see `docs/20_data_formats.md`).

## Gamepad-triggered hot join: `PendingJoinGamepad`

<!-- b:1606.rule -->
<!-- b:1606.ref -->

**Gamepad-triggered hot join** (`gamepad_hot_join.md`) adds a second, *global* gamepad-binding
surface alongside the per-player `InputMap.gamepad_*` fields (described earlier on this page) — `ProjectGamepadBindings`/
`LoadedGamepadBindings` (`runtime/scene_manager/mod.rs`), populated from
`ProjectConfig.global_unclaimed_gamepad_bindings`/`GameSceneV2.scene_unclaimed_gamepad_bindings` at exactly the three
sites `ProjectKeyBindings`/`LoadedKeyBindings` already use (two in `project_loader.rs`, one in
`scene_loader.rs`), same per-key overlay semantics. `unclaimed_gamepad_trigger_system`
(`runtime/input.rs`, `.before(fsm_interpreter_system)`) checks these bindings only against
gamepads **not** already claimed by a live player's `BoundGamepad`, by an undrained `is_hot_join`
entry's own captured `PlayerConfig.bound_gamepad` in `PendingEntitySpawns`, or by a still-pending
live player's own seed resolving to that pad this same frame (the last case exists because
`gamepad_bind_system` — the only system that actually writes a *resolved* `BoundGamepad` — runs in
`FixedUpdate`, which may not tick this frame at all; without it a pad could look unclaimed for one
frame right as an authored player's seed is about to claim it, debug-detective finding). All three
are `Entity`-based, not the pre-hardening positional `HashSet<usize>` derived from live
`gamepad_index` — that set went stale the instant any pad connected/disconnected mid-session — on
a `just_pressed` match (no separate
"live signal" prefilter: a phantom/dead duplicate pad, (the troubleshooting note this pointed to was never written down), never
produces that edge on anything) it emits the usual `UiEvent::ButtonPressed` **and** writes the
matched gamepad's `Entity` into a new `PendingJoinGamepad(Option<Entity>)` resource — at most one
pad captured per frame (deterministic: lowest `Entity::index()`-sorted), reset to `None`
unconditionally at the top of every run so a non-join gamepad trigger (e.g. a pause button) can
never leave a stale pad identity for a later frame's keyboard-triggered join to inherit.
`Action::JoinPlayer`'s executor arm (site 5 in `docs/dev/player-spawn-sites.md`) `.take()`s this
resource after resolving the joiner's `PlayerConfig` and, if set, writes it directly into
`PlayerConfig.bound_gamepad` — no round-trip through `inputs.gamepad_index`/`resolve_gamepad` (the
pre-hardening design re-resolved a converted sorted index back to an `Entity` ≥1 frame later, a
window any pad churn could exploit to bind the wrong device). `spawn_player_entity_core` inserts
`BoundGamepad(player_config.bound_gamepad)` instead of always `None` for this one call path. A
keyboard-triggered join sees the resource already `None` and is unaffected. This override does
**not** disable the joiner's keyboard scheme — gamepad and keyboard inputs are read additively
(`||`), never exclusively, everywhere in this file's gamepad routing on this page.
