//! The declared order of the message -> interpreter -> executor pipeline (D3,
//! `planning/features/done/gameplay_pipeline_system_sets.md`).
//!
//! **Unstable:** these sets are `pub` only so tests (and the future two-`App` determinism test) can read the graph; do not
//! order gameplay systems against them from outside `ironhold_core`.
//!
//! ```text
//! Update:  Emit  ->  Interpret  ->  Execute  ->  PostExecute
//!
//! Emit:    Scene  -> { Ui, Game } -> DirectActions -> Dialogue
//!          Ui   = global_input -> unclaimed/claimed gamepad -> button -> icon_button
//!          Game = Targeting -> ActionBar -> Interact -> DelayedEvents -> Backend -> Stats   (GameStreamSet, chained)
//! ```
//!
//! # Why
//! The FSM takes the *first matching transition* per event and applies it immediately, so the order in which same-frame
//! events and actions reach the interpreters can change the final `LogicState`. Before D3 the writers of
//! `UiEvent`/`GameEvent`/`SceneEvent` and the direct `ActionQueue` pushers were ordered against the interpreter but not
//! against *each other*: native (multi-threaded) and web (single-threaded) could disagree, and so could two native runs.
//!
//! # Rules
//! - **Every `Update` system that writes `UiEvent`/`GameEvent`/`SceneEvent`, pushes `ActionQueue` or writes `SimClock`
//!   (or reads those messages) belongs to exactly one of these sets** (the most specific one, never also directly to
//!   `GameplaySet::Emit`). `tests/schedule_order_tests.rs` checks this from each system's real access, so writers hidden
//!   inside `SystemParam` bundles are seen too, and fails on any unordered conflict on those resources.
//! - **Never write `.before/.after(stat_effective_value_system)`.** It is registered twice (once in `GameStreamSet::Stats`,
//!   once in `GameplaySet::Execute`) and Bevy refuses to build a schedule that orders against a function that has two
//!   instances (`SystemTypeSetAmbiguity`). Order against `GameStreamSet::Stats` or `GameplaySet::Execute` instead.
//! - Run conditions (`sim_running`, `any_action_slots`, `resource_changed::<SimClock>`) stay on their systems: Bevy
//!   evaluates a *set* condition once, at the first member's turn, and skips every member, which would change behaviour.
//! - `keep_messages_flowing_while_held` stays in `Last`, outside every gameplay set: it must run after every `Update`
//!   reader and before `First`'s message update, touches only `MessageRegistry` and is not a writer. When these sets move
//!   to `FixedUpdate` (the fixed-tick plan) it must stay in `Last`.
//! - Accepted, unordered: first-person and flycam look (`first_person_camera_system`, `fly_camera_system`) read `SimClock`
//!   inside the camera chain, which is not ordered against the clock's writers (`spawn_scene_v2`, `action_executor_system`);
//!   the deferred `CameraChainSet` follow-up orders that chain after `GameplaySet::Execute` and removes the allowlist.
//! - **The pause clock** (`SimClock`) is written by `spawn_scene_v2` (`EmitSet::Scene`, the Replace-load auto-resume) and
//!   `action_executor_system` (`GameplaySet::Execute`); `apply_sim_clock_system` runs right after the executor, the last
//!   writer of the frame, and is the only caller of `Time<Virtual>::pause()`/`unpause()`.
//!
//! # Order the docs promise (designer view: `docs/30_runtime_events_and_logic.md`, "Same-frame event order")
//! - `ActionQueue` push order in a frame: `DirectActions` (behaviour activation, despawn timers) -> `Dialogue` -> FSM ->
//!   entity FSM -> action-bar flush. An action-bar slot's built-in `do_actions` therefore run *after* every
//!   `state_machine.ron` reaction, and dialogue actions run *before* them.
//! - The game stream the interpreters read in frame N: (1) the carry-over from frame N-1's `Execute` (action-bar
//!   `activated`, executor events, then `game.paused`/`game.resumed`, then stat-threshold crossings), (2) events from the
//!   `FixedUpdate` tick(s), (3) `EmitSet::Game` in its chain order. Stat thresholds and the pause events are always
//!   interpreted one frame after the action that caused them.
//! - Accepted consequences: the action-bar cost gate sees the previous frame's regen (`.current`); a behaviour that
//!   finishes loading reacts to this frame's events; on a Replace-load frame every input system sees the new scene's
//!   bindings and the released clock, while **time resumes the frame after** (`Time<Virtual>` is unpaused in `Execute`).
//!
//! # Observation points for a two-`App` determinism test (D5)
//! A recorder reading `MessageReader<UiEvent|GameEvent|SceneEvent>` placed `.in_set(GameplaySet::Interpret)
//! .after(entity_fsm_interpreter_system)` sees the streams in the order the interpreters read them; an `ActionQueue`
//! snapshot taken `.after(flush_pending_intent_system).before(action_executor_system)` sees the frame's complete push
//! order. Both reference single-instance systems, so by-name edges are legal there.

use bevy::prelude::*;

/// The four phases of the per-frame pipeline, chained in `Update`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameplaySet {
    /// Everything that *produces* messages or direct actions for the interpreters.
    Emit,
    /// `fsm_interpreter_system` -> `entity_fsm_interpreter_system`.
    Interpret,
    /// `flush_pending_intent_system` -> `action_executor_system` -> `apply_sim_clock_system` -> `stat_effective_value_system`
    /// (2nd instance) -> `stat_threshold_system` -> the drain/pool/decal tail.
    Execute,
    /// Readers of what `Execute` produced (`npc_hit_relay_system`, inventory/container UI, audio mirrors, scene-event readers).
    PostExecute,
}

/// The independent streams inside [`GameplaySet::Emit`].
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmitSet {
    /// `check_project_loaded` -> `spawn_scene_v2`: runs first so input on a load frame sees one consistent world.
    Scene,
    /// `UiEvent` writers.
    Ui,
    /// `GameEvent` writers, ordered by [`GameStreamSet`].
    Game,
    /// Direct `ActionQueue` pushers that are not interpreters: behaviour activation, despawn timers.
    DirectActions,
    /// `dialogue_tick_system`: reads both streams and pushes actions, so it runs last in `Emit`.
    Dialogue,
}

/// The chained sub-streams of [`EmitSet::Game`].
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameStreamSet {
    Targeting,
    ActionBar,
    Interact,
    DelayedEvents,
    /// `backend_glue_system`: online replies become `GameEvent`s and engine-written variables.
    Backend,
    Stats,
}

/// Defines the sets and their order. Systems join them with `.in_set(...)` at their own registration sites, so each
/// system keeps its registration position (insertion order breaks ties on the single-threaded web executor; a few
/// registrations were merged into set tuples, which is harmless for pipeline systems because their order is fixed by edges).
/// `tests/schedule_order_tests.rs` is the manifest of which systems belong to which set. The plugin must be added
/// together with the plugins whose systems are members (`ActionBarPlugin`, `TargetingPlugin`): without it the sets exist
/// but are unordered. Adding it first is only tie-break hygiene.
pub struct GameplaySchedulePlugin;

impl Plugin for GameplaySchedulePlugin {
    fn build(&self, app: &mut App) {
        // Normal edges between the four phases: the sync point before `Interpret` is required so commands queued during
        // `Emit` (a freshly loaded behaviour, a despawn) are applied before the interpreters run.
        app.configure_sets(
            Update,
            (
                GameplaySet::Emit,
                GameplaySet::Interpret,
                GameplaySet::Execute,
                GameplaySet::PostExecute,
            )
                .chain(),
        );

        // Inside `Emit`: `Scene` first (normal edges: one sync so Ui/Game see the new scene's bindings and the released
        // clock); `Ui` and `Game` share no message resource so stay independent; the rest use `*_ignore_deferred` so the
        // intra-`Emit` command visibility is unchanged (their commands still flush at the next normal edge).
        app.configure_sets(
            Update,
            (
                EmitSet::Scene.in_set(GameplaySet::Emit),
                EmitSet::Ui.in_set(GameplaySet::Emit).after(EmitSet::Scene),
                EmitSet::Game.in_set(GameplaySet::Emit).after(EmitSet::Scene),
                EmitSet::DirectActions
                    .in_set(GameplaySet::Emit)
                    .after_ignore_deferred(EmitSet::Ui)
                    .after_ignore_deferred(EmitSet::Game),
                EmitSet::Dialogue
                    .in_set(GameplaySet::Emit)
                    .after_ignore_deferred(EmitSet::Ui)
                    .after_ignore_deferred(EmitSet::Game)
                    .after_ignore_deferred(EmitSet::DirectActions),
            ),
        );

        // The game stream, in the order the interpreters should see it: input first, then delayed events, then stat events.
        app.configure_sets(
            Update,
            (
                GameStreamSet::Targeting,
                GameStreamSet::ActionBar,
                GameStreamSet::Interact,
                GameStreamSet::DelayedEvents,
                GameStreamSet::Backend,
                GameStreamSet::Stats,
            )
                .chain_ignore_deferred()
                .in_set(EmitSet::Game),
        );
    }
}
