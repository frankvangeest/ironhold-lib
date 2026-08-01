# Feature: Gamepad Player-Binding Hardening

_Status: Revised after plan-review — recommend a confirmation pass before implementation_
_Planned at: `1fcef14` (2026-07-31)_
_Revised at: `2026-08-01` (system-architect verified the Bevy claim against `bevy_gilrs` too, not
just `bevy_input`, and found the platform-dependent identity caveat, the cross-time double-bind
race, the `camera_orbit_system`/`OrbitCamera.gamepad_index` second-source-of-truth bug, and the
`unclaimed_gamepad_trigger_system`/hot-join `claimed`-set gap; ux-gamedesigner-reviewer resolved
the duplicate-detection open question to both warn+hard-error and found the per-catalog
false-positive risk plus the missing docs/demo scope)_

## What
Fixes two related gamepad-robustness gaps in local co-op surfaced during this session's gamepad
features (`gamepad_controller_input.md`, `gamepad_action_bar_slots.md`, `gamepad_hot_join.md`):

1. **`InputMap.gamepad_index` is a live *positional* index, re-resolved fresh every frame against
   whatever gamepads happen to be connected right now** — a mid-session disconnect of any
   lower-sorted pad shifts every higher pad's position down by one, silently re-routing a still-live
   player's input to a *different physical device*, or causing one player's press to fire another
   player's ability/movement/camera.
2. **Two players can be authored with the same `gamepad_index` with no detection at all** — one
   physical pad press then drives both players simultaneously, undetected by anything.

## Why
Both gaps are logged in `planning/backlog.md` (`Positional gamepad_index → resolved-Entity
binding`; `Duplicate InputMap.gamepad_index across players is never detected`), the first
confirmed as a *worsening* problem across three features in this session — first flagged as a
camera/movement oddity during `gamepad_controller_input.md`'s playtest, then confirmed by
`gamepad_hot_join.md`'s review to also risk a spurious extra join, then confirmed by
`gamepad_action_bar_slots.md`'s review to be able to fire *a different player's ability slot*
outright. Each feature that added a new gamepad-consuming system inherited the same latent
fragility rather than it being fixed once. `planning/claude_suggestions.md` also documents the
concrete triggering incident: a real Xbox 360 controller registered as two separate browser
gamepad entries during `gamepad_controller_input.md`'s hardware playtest, one of them permanently
dead — this is a real, reproducible hardware quirk, not a hypothetical.

## Approach

### Investigation, grounded in Bevy 0.18's actual gamepad-disconnect behavior (verified against
`bevy_input-0.18.0/src/gamepad.rs`, not assumed)

**A disconnected gamepad's `Entity` is never despawned — only its `Gamepad` component is
removed, and re-added to the *same* entity on reconnect** (`gamepad_connection_system`,
`bevy_input-0.18.0/src/gamepad.rs:1506-1545`; the doc comment states this explicitly: "Entities
are left alive to preserve their state... instead of despawning, we remove Gamepad components...
and re-add them if they ever reconnect"). This is the key fact that makes an `Entity`-based
binding strictly better than today's positional one, not just structurally cleaner:
- **On disconnect**: a query for `&Gamepad` against the bound `Entity` simply stops matching (the
  component is gone) — this degrades to exactly the same "no gamepad input this frame" behavior
  every consuming system already has for an out-of-range positional index. No new failure mode.
- **On reconnect of the *same physical device***: Bevy re-inserts `Gamepad` onto that *same*
  `Entity` — an `Entity`-bound player's gamepad **automatically resumes working**, with zero
  additional code. This is a genuine UX improvement over today's design, not merely a fix — today,
  a disconnect/reconnect cycle can permanently shift every subsequent player's positional index by
  one, with no self-correction.
- **A late-connecting gamepad** (not yet present at the moment a player is meant to bind) needs an
  explicit "still pending" retry state — a one-time positional resolution can't be a single
  spawn-time snapshot, or a player who spawns before plugging in their controller would never bind
  at all (a real regression versus today's every-frame re-derivation). See the `BoundGamepad`
  design below.

### New component: `BoundGamepad(pub Option<Entity>)`

Inserted on every player entity at spawn time (all five player-construction sites — see
`crates/ironhold_core/src/CLAUDE.md`'s inventory — must insert it, mirroring how `PlayerIndex`/
`PlayerTarget` already need to be threaded through every site). Two states:
- **`BoundGamepad(None)`** — "pending": either no `gamepad_index` was authored, or the seed index
  had no live gamepad at the moment binding was last attempted. A new system (or a branch folded
  into an existing per-frame system — TBD during implementation) re-attempts the *existing*
  `resolve_gamepad(sorted_gamepads, seed_index)` positional lookup **only while pending**, using
  the RON-authored `gamepad_index` purely as a one-time seed; the moment it resolves to `Some`,
  the result is written into `BoundGamepad` and never re-derived again for that player's lifetime.
- **`BoundGamepad(Some(entity))`** — "bound": locked to that specific `Entity` forever (barring a
  future hot-leave/rejoin). All five gamepad-consuming systems
  (`input_translator_system`, `tab_targeting_system`, `interactable_system`, `camera_orbit_system`,
  `action_bar_input_system`) stop calling `resolve_gamepad` for a *bound* player and instead do a
  direct `gamepad_query.get(bound.0).ok()` against the stored `Entity` — no re-sorting, no
  re-deriving position, immune to any other pad's connect/disconnect churn.

`resolve_gamepad` itself is kept, unchanged, for the two remaining genuinely-positional use cases
that have no "already bound" concept yet: (a) the pending-bind retry above, and (b)
`unclaimed_gamepad_trigger_system`'s hot-join pad-capture logic (`PendingJoinGamepad`), which
inherently needs live positional/press-based detection since there's no player entity — let alone
a `BoundGamepad` component — to bind onto until the join actually happens.

**Revision (2026-08-01, after plan-review — system-architect + ux-gamedesigner-reviewer):** both
reviews found real gaps in the original draft above; incorporated below rather than left as
findings to rediscover during implementation.

**Platform caveat on the Bevy claim (system-architect, verified against `bevy_gilrs-0.18.0`, not
just `bevy_input`) — record this explicitly, don't oversell "same physical device forever."**
"Same `Entity` on reconnect" strictly means "same `gilrs::GamepadId`," and *what* counts as the
same id is platform-dependent: Linux matches by device UUID (true device identity), but **Windows
XInput and Web (`web_sys` `Gamepad.index()`) both match by slot/index, not device** — the two
platforms this project actually targets. Consequence: a *different* controller plugged into a
freed slot inherits the previous device's `Entity` (rare, but exactly the failure mode this plan
exists to eliminate — state it rather than imply the fix is airtight); a device reconnecting at a
*different* slot becomes a **new**, un-bound `Entity`, and per this plan's own "no auto re-bind"
scope decision, that player has no recovery path. The honest framing is *predictable-and-inert*
replacing *unpredictable-and-sometimes-lucky* — still the right trade, but say so precisely.
Add to "Explicitly out of scope": a one-shot `warn!` when a bound player's `Gamepad` has been
absent for N seconds ("P2's controller disconnected — reconnect it to the same port/slot to
resume") turns an otherwise-silent dead player into a diagnosable one; cheap, worth adding as a
task even though full reconnect-at-a-new-slot recovery stays out of scope.

**BLOCKER, resolved — the real race is cross-*time*, not cross-player (system-architect).** Two
distinct seeds can never resolve to the same pad *at the same instant* (`resolve_gamepad` is
injective over one fixed slice), so the plan's original "what if two seeds resolve to the same pad"
framing was asking the wrong question. The actual race: pad B is the only one connected at launch;
P1 (seed 0) binds to B; P2 (seed 1) is out-of-range, stays pending. Later, pad A connects with a
**lower** `Entity::index()` than B. Sorted slice is now `[A, B]`. P2's next retry resolves seed 1
→ **B** — the pad P1 already holds. Pad A is never claimed by anyone. Both seeds are distinct and
authored correctly, so neither the new duplicate-`gamepad_index` warn nor the CLI check catches
this. **Fix, now a hard invariant**: the pending-bind retry must never write `BoundGamepad(Some(e))`
for an `e` any other player's `BoundGamepad` already holds — the retry system needs visibility into
every player's current binding in one pass (see "where the retry lives," below), not just its own
per-player loop. **Decision on what a displaced pending player does**: stay pending (no rebind to a
different, already-free pad this session) + the one-shot disconnect-style `warn!` above once it's
clear the seed will never resolve to a free pad. (The alternative — demoting `gamepad_index` from a
literal position to a claim-order *preference*, so a displaced pending player claims the lowest
*unbound* pad instead — was considered and rejected for v1 of this hardening: it changes what
`gamepad_index` *means* semantically, which needs its own doc/UX pass, and the simpler "stay
pending, diagnosable via warn" behavior is a strict improvement over today's silent mis-routing
either way.)

**Where the retry lives — a dedicated system, not folded into an existing one (system-architect,
resolving the plan's own open question).** Three reasons: (1) the cross-player invariant above
needs to see every player's `BoundGamepad` in one pass — awkward to state correctly if binding
logic is spread across per-player branches inside another system; (2) it keeps the five gamepad
consumers uniformly simple (`bound.0.and_then(|e| gp_q.get(e).ok())`, no lifecycle branch); (3) none
of the five currently have the `Commands`/write access this needs, and bolting lifecycle mutation
onto a system whose job is "translate this frame's input" muddies a clean single-purpose system to
avoid one `.before()` ordering edge. New `gamepad_bind_system` in `runtime/input.rs`, ordered
`.before(input_translator_system)`.

**BLOCKER, resolved — `camera_orbit_system` cannot simply "read `BoundGamepad`" (system-architect).**
Unlike the other four consumers, it has no player `Entity` in scope — it resolves gamepad input via
`orbit.gamepad_index`, a value **pre-resolved onto the `OrbitCamera` component at spawn time**
(`camera.rs`), and its only link to the player is `character_query: Query<&mut Transform, With<
CharacterController>>` (Transform only, no `BoundGamepad` access). Left as-is, `OrbitCamera.
gamepad_index` becomes a **second, spawn-frozen copy of the binding** that can silently diverge
from `BoundGamepad` — reintroducing this plan's own "one source of truth" thesis as a bug one layer
over. **Fix, now a named task**: add a read-only `bound_q: Query<&BoundGamepad>` to
`camera_orbit_system` (disjoint from the existing `&mut Transform` access, no borrow conflict),
resolve via `bound_q.get(orbit.target)`, and **delete `OrbitCamera.gamepad_index`** plus its
spawn-time resolution site entirely. Keep `OrbitCamera.gamepad_deadzone` (a genuine per-camera
tuning value, not a binding).

**BLOCKER, resolved — `unclaimed_gamepad_trigger_system`'s "claimed" set and the hot-join hand-off
must both move to `Entity`, not stay index-based (system-architect).** The plan's original draft
said hot-join's capture mechanism is "independent of this refactor" — true only for the *capture*
(`PendingJoinGamepad`, a `just_pressed`-on-an-unclaimed-pad detector, genuinely unaffected). Two
things downstream of it are not independent:
- `unclaimed_gamepad_trigger_system`'s `claimed: HashSet<usize>` (`runtime/input.rs`) is currently
  derived from live `CharacterController.inputs.gamepad_index` — once that field is a **seed**, not
  a **binding**, this set stops describing reality the moment any pad connects/disconnects. Worked
  failure (hot-leave interaction, if `local_coop_hot_join_leave.md` ships first): P1 bound to A
  (seed 0), P2 bound to B (seed 1); P1 hot-leaves. Sorted becomes `[B]`, but `claimed` (still
  index-derived from the one remaining `CharacterController`) reads `{1}` — position 0 (= B, P2's
  own live pad) now looks unclaimed, so pressing the join button on **P2's own controller** spawns
  a spurious P3. This is the exact "known accepted hazard" this codebase's own doc comment already
  named as *"the real fix is an `Entity`-resolved binding, not a positional index"* — and this plan
  builds that primitive and would otherwise decline to apply it to the one place that comment was
  about. **Fix**: change `claimed` to `HashSet<Entity>` sourced from every live player's
  `BoundGamepad`.
- The hot-join hand-off itself round-trips `Entity → index → Entity`
  (`action_executor.rs`: captured `PendingJoinGamepad` `Entity` → converted to a sorted index →
  stored in `player_config.inputs.gamepad_index` → later re-resolved back to an `Entity` by the new
  pending-bind retry, ≥1 frame later since spawns are queued). Any pad churn in that window binds
  the joiner to the wrong device, in the one path where the correct `Entity` was already known
  exactly. **Fix**: add `bound_gamepad: Option<Entity>` to `PlayerConfig` (legal — `PlayerConfig` is
  not `Deserialize`, confirmed, so this is a pure-Rust runtime field, no schema/RON exposure), set
  it directly from the captured `PendingJoinGamepad` entity, and have `spawn_player_entity_core`
  insert `BoundGamepad(player_config.bound_gamepad)` instead of always `None` for a hot-joined
  player — closing the round-trip entirely rather than narrowing its window.

### Duplicate detection — **resolved to both a `warn!` and a hard `ironhold_cli validate` error**
(not left as an open question — see reasoning below), **scoped per-scene, not per-catalog**

New scene-load `warn!` in `spawn_players_and_camera` (mirroring
`warn_missing_player_stat_templates`'s exact shape) **plus** a matching hard `ironhold_cli
validate` error: two or more player-tagged prefabs **instantiated in the same scene's `entities:`
list** authoring the same non-`None` `gamepad_index` value. **Must resolve scene-instantiated
players, not the raw prefab catalog** — `local_coop_demo`'s catalog already contains legitimate
catalog-level duplicates (`player_p1_split`/`player_p1_split_ring` both author `gamepad_index: 0`,
similarly for P2 — different rooms' variants, never co-instantiated); a catalog-wide check would
false-positive on an already-shipped project and break `cargo test -p ironhold_cli --test
validate_projects`. Add `local_coop_demo` as the explicit negative-case test.

**Why both a warn and a hard error, not just a warn (resolving the plan's original open question):**
this mistake is invisible without two physical controllers (gamepad input is purely additive, so
keyboard-only testing — the common case — can't surface it), the realistic path into it is this
project's own documented troubleshooting advice ("try `gamepad_index: 1` or `2`" for the ghost-pad
quirk, which a designer with two players could easily apply to both), and the failure mode reads as
"the game is broken" (both viewports move in lockstep) rather than "I made an authoring mistake" —
worse to leave silent than the already-precedented `gamepad_key_without_gamepad_index` check, which
gets the same warn+hard-error treatment for a strictly milder failure mode (inert, not
mis-behaving). The error message should name the one deliberate-looking case it forecloses:
`"...both use gamepad_index: 1 — one physical controller would drive both characters at once. Give
each player a different gamepad_index. Deliberately sharing one controller between two characters
is not supported."` **Note this check is largely subsumed by the B1 cross-time-race fix above**
once "never bind an already-bound `Entity`" is a hard invariant — a duplicate seed can no longer
produce silent dual-control at runtime (the second player just stays pending), so the warn/error
becomes purely explanatory rather than the only thing standing between a designer and broken
dual-control.

## Explicitly out of scope
- Auto re-binding a *different* new gamepad that connects after a player is already bound — once
  bound, a player's binding is locked to that specific `Entity` for their whole lifetime; picking
  up a second, later-connected pad is not a goal here. (A one-shot diagnostic `warn!` after N
  seconds of absence *is* in scope — see above; full recovery is not.)
- Any UI for a player to manually pick/rebind their gamepad — no rebinding UI exists anywhere in
  this engine today; out of scope for this hardening pass.
- Hot-leave-triggered-by-disconnect (a bound player's pad disconnecting does **not** despawn the
  player or trigger `local_coop_hot_join_leave.md`'s hot-leave, if that feature ships first) — a
  disconnected-but-still-`BoundGamepad`'d player simply stops receiving gamepad input (their
  keyboard scheme, if any, keeps working, since gamepad input is always additive in this engine).
- **Runtime observability of binding state (e.g. a `player.{index}.gamepad_connected` `GameVariable`,
  or `input.gamepad_disconnected`/`reconnected` events a designer could bind a "Controller
  disconnected" banner to)** — "pending" and "bound-but-disconnected" become well-defined engine
  states after this plan, but stay completely unreachable from RON; a real commercial-couch-co-op
  UX gap, logged here so it isn't rediscovered as a surprise in a future gamepad feature, not
  attempted in this pass.

## Tasks
- [ ] New `BoundGamepad(pub Option<Entity>)` component (`capabilities/player.rs`, alongside
      `PlayerIndex`/`PlayerTarget`)
- [ ] Insert `BoundGamepad(None)` at all five player-construction sites, **except** the hot-join
      site, which instead inserts `BoundGamepad(player_config.bound_gamepad)` (see the new
      `PlayerConfig.bound_gamepad` field below) — same shared insertion point in
      `spawn_player_entity_core`'s post-dispatch code as `PlayerIndex`/`PlayerTarget`
- [ ] New `PlayerConfig.bound_gamepad: Option<Entity>` (not `Deserialize`-exposed — pure runtime
      field), set directly from the captured `PendingJoinGamepad` entity in `Action::JoinPlayer`'s
      executor arm, replacing the current `Entity → index → Entity` round-trip through
      `player_config.inputs.gamepad_index`
- [ ] New dedicated `gamepad_bind_system` (`runtime/input.rs`, `.before(input_translator_system)`):
      for every player with `BoundGamepad(None)` and a `Some(gamepad_index)` seed, attempt
      `resolve_gamepad` against the current frame's sorted slice; **enforce the hard invariant that
      it never binds an `Entity` another live player's `BoundGamepad` already holds** (requires
      seeing every player's binding in one pass, hence a dedicated system, not a per-player-loop
      branch folded into an existing consumer); on success, write `BoundGamepad(Some(entity))` and
      never re-attempt for that player again
- [ ] One-shot diagnostic `warn!` when a bound player's `Gamepad` component has been absent for N
      seconds (exact threshold TBD during implementation), naming the player and suggesting
      reconnecting to the same port/slot
- [ ] Refactor `input_translator_system`, `tab_targeting_system`, `interactable_system`,
      `action_bar_input_system` to read `BoundGamepad` directly (`gamepad_query.get(bound.0?).ok()`)
      instead of calling `resolve_gamepad` with a live positional index/sorted slice
- [ ] `camera_orbit_system`: add a read-only `Query<&BoundGamepad>` resolved via `orbit.target`,
      and **delete `OrbitCamera.gamepad_index`** (its spawn-time-frozen positional resolution) along
      with the spawn-site code that populates it — this field would otherwise become a second,
      silently-divergent copy of the binding (keep `OrbitCamera.gamepad_deadzone`, a genuine
      per-camera tuning value, unaffected)
- [ ] `unclaimed_gamepad_trigger_system`: change its `claimed: HashSet<usize>` (positionally
      derived from live `CharacterController.inputs.gamepad_index`) to `HashSet<Entity>` sourced
      from every live player's `BoundGamepad` — required so a hot-leave (if
      `local_coop_hot_join_leave.md` ships first) doesn't make a still-claimed pad look unclaimed
      and trigger a spurious join
- [ ] Duplicate-`gamepad_index` detection, **scoped to a scene's instantiated `entities:` list, not
      the raw prefab catalog**: `warn!` in `spawn_players_and_camera` + a hard `ironhold_cli
      validate` error (both, not just a warn — see Approach's reasoning), with an error message
      naming "deliberately sharing one controller between two characters is not supported"
- [ ] Tests — regression: a scene with no `gamepad_index` authored anywhere behaves byte-for-byte
      identically; `local_coop_demo`'s existing catalog-level `gamepad_index` duplicates (different
      rooms' prefab variants) still validate clean — the explicit negative case for the
      per-scene-not-per-catalog scoping; new: a player spawned with a gamepad already connected
      binds immediately and stays bound to that exact `Entity` even after an unrelated *other* pad
      disconnects/reconnects mid-session; new: a player spawned *before* their gamepad connects
      later successfully binds once it does; new: a disconnected-then-reconnected *same* physical
      pad's bound player automatically resumes receiving input with zero extra code; new: **the
      cross-time double-bind race** — pad B connects first and binds to P1 (seed 0), P2 (seed 1)
      stays pending, pad A then connects with a lower `Entity::index()` — P2 must NOT bind to B
      (the invariant this plan's B1 fix exists for); new: two players authored with the same
      `gamepad_index` in one scene trigger the new `warn!`/validate error; new: hot-leave then a
      pad-press on the surviving player's own controller does NOT spawn a spurious join (the
      `claimed`-set fix); new: a hot-joined player's captured pad binds via `PlayerConfig.
      bound_gamepad` with no positional round-trip window
- [ ] Docs — `docs/20_data_formats.md`: update `gamepad_index`'s field description plus a new
      "How a controller gets assigned to a player" prose note beneath the `CameraConfig`-adjacent
      table (observable-behavior wording, no "Entity"/"component" implementation vocabulary); update
      the existing ghost-duplicate troubleshooting box (the consequence of a duplicate vanishing
      mid-session changes under this plan); update the hot-join section's description of the pad
      hand-off; update the `gamepad_key` note's "owner_player → that player's InputMap.gamepad_index"
      phrasing to "that player's own controller"
- [ ] Docs — RON comments: the `gamepad_index` explainer comments in `local_coop_demo`'s
      `prefabs.ron` (5 sites) and `entity_logic_demo`'s `prefabs.ron` (1 site) currently describe
      only additivity; add the bind-once-then-lock behavior to the canonical live example
      (`player_p1_split`)
- [ ] Docs — `crates/ironhold_core/src/CLAUDE.md`: update the "Gamepad routing" section's
      description of `resolve_gamepad` to describe the new split (bound players use `BoundGamepad`
      directly; `resolve_gamepad` remains only for the pending-bind retry and hot-join's pad
      capture); update the "Gamepad-triggered hot join" section, which currently documents the
      index round-trip as the mechanism
- [ ] Demo — add one `Label` to `local_coop_demo/room3.scene.ron` (already the canonical live
      two-pad room — `player_p1_split`/`player_p2_split` on `gamepad_index: 0`/`1`) inviting a
      controller-unplug/replug test, rather than a new room — e.g. `"Controller test: unplug/replug
      a pad mid-game - it stays with the same player"`

## Open questions
- **(resolved, post-review)** Duplicate-`gamepad_index` enforcement: both a `warn!` and a hard
  `ironhold_cli validate` error, scoped per-scene (not per-catalog) — see Approach.
- **(resolved, post-review)** Pending-bind retry location: a new dedicated `gamepad_bind_system`,
  not folded into an existing consumer — required by the cross-player invariant (B1) below.
- **(resolved, post-review)** Sorted-slice removal: 4 of the 5 gamepad consumers can drop it
  entirely once bound; `camera_orbit_system` needs its own fix first (deleting `OrbitCamera.
  gamepad_index`, reading `BoundGamepad` via `orbit.target` instead) — see Approach.
- **(open, low priority)** Exact "absent for N seconds" threshold for the one-shot disconnect
  diagnostic `warn!` — pick a reasonable default (e.g. 3s) during implementation; not
  design-critical.

## Acceptance criteria
- Given a 2-player local-coop scene where both players are bound to distinct connected gamepads,
  when a third, unrelated gamepad connects then disconnects mid-session, then neither player's
  input is affected (**regression**).
- Given a player bound to gamepad seed index `1`, when the gamepad currently occupying sorted
  position `0` disconnects mid-session, then the player's own physical pad keeps driving them
  correctly — no re-routing to a different device (**the core bug fix, browser-observable with two
  real controllers**).
- Given the same scenario, when the disconnected pad reconnects **to the same slot/port**, then the
  player who was bound to it automatically resumes receiving input with no re-join/reload needed
  (documented as slot-based on Windows/Web, not guaranteed-same-physical-device — see the platform
  caveat in Approach).
- Given a player bound to a pad that is then unplugged, when they use their authored keyboard
  bindings, then they still move — gamepad loss never freezes a player who has a keyboard scheme.
- Given `local_coop_demo` room3 loaded fresh in a browser with two controllers already connected,
  when each player presses a button (browsers don't expose a pad until first input), then each
  binds to their own authored seed and moves only their own character.
- Given a player authored with a `gamepad_index` seed that never resolves to a connected pad, then
  no per-frame warning/log spam occurs — they simply play on keyboard, silently pending.
- Given `local_coop_demo` room8, when a gamepad-joined player's pad is unplugged and replugged and
  they press the join button again, then **they resume control of their own existing player** — no
  additional player spawns (the `claimed`-set fix, browser-observable with hardware).
- Given a scene with two players authored with the same `gamepad_index` **in that scene**, when the
  scene loads, then a clear `warn!` fires and `ironhold_cli validate` reports a matching hard error
  — no crash, no silent dual-control.
- Given `local_coop_demo`'s existing catalog (which legitimately reuses `gamepad_index` values
  across different rooms' prefab variants), when validated, then it still passes clean — the
  per-scene scoping regression.
- Given a scene with no `gamepad_index` authored on any player, when this ships, then behavior is
  byte-for-byte unchanged from today.
- **Minimum hardware note**: full verification of the above needs two physical controllers; with
  one pad, the disconnect/reconnect criteria are only partially observable and the remainder is
  covered by the automated tests.
