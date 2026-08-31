//! End-to-end coverage for `planning/features/monster_corpse_loot.md` v2's interact→loot flow
//! and the death→corpse-swap mechanism `at_entity` was added to support.
//!
//! Originated as a debug-detective reproduction of a v1 playtest report ("kill zombie, press F
//! near corpse, container panel does not open") that turned out to have an environmental cause (a
//! stale dev server serving the wrong worktree, not a code bug) — kept as permanent regression
//! coverage since no other test exercises the real interact-key-press → loot-panel vertical slice.
//! Rewritten for v2: a monster's `interactable`/`inventory` now live on a *separate* corpse
//! prefab, spawned via `Action::Spawn(..., at_entity: "{self}")` when the monster dies, so the
//! tests below cover both halves — the corpse's own loot/decay behavior, and the monster's
//! death → corpse-swap → despawn sequence that produces it.
//!
//! Loads the REAL `3rd_person_game_demo` RON (prefabs.ron, assets.ron, enemy_zombie.behavior.ron,
//! lootable_corpse.behavior.ron) so the authored radii/loot/state names are exercised, then drives
//! the real systems — e.g. for the swap:
//!   ModifyStat -> stat_threshold_system -> entity_fsm_interpreter_system (-> "dead")
//!   EmitEventAfterDelay -> tick_delayed_events_system -> entity_fsm_interpreter_system
//!     -> action_executor_system (Despawn/Spawn with at_entity) -> drain_spawn_queue_system
//! Asserts on real ECS/resource state (SpawnRegistry, Transform, LoadedContainerUi, Visibility),
//! not on cumulative message buffers.

use bevy::prelude::*;
use ironhold_core::capabilities::action_bar::CurrentTarget;
use ironhold_core::capabilities::animation::AnimationController;
use ironhold_core::capabilities::animation_resolver::{
    ActiveOverride, AnimationPolicyComponent, AnimationRequests, LocomotionState,
    resolve_initial_override,
};
use ironhold_core::capabilities::interactable::Interactable;
use ironhold_core::capabilities::inventory::{
    ContainerPanelMarker, Inventory, LoadedContainerUi, LoadedInventoryUi, add_to_slots,
};
use ironhold_core::capabilities::player::{CharacterController, PlayerTarget};
use ironhold_core::runtime::{
    ActionQueue, BehaviorHandle, EntityFsmState, LoadedAssetCatalog, LoadedPrefabCatalog, SpawnId,
    SpawnRegistry,
};
use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog};
use ironhold_core::schema::player::{AnimationPolicy, InputMap};
use ironhold_core::schema::stats::{LiveStat, StatDef, StatMap};
use ironhold_core::schema::{Action, StateMachineAsset};

mod support;
use support::setup_test_app;

const DEMO: &str = "../../assets/projects/3rd_person_game_demo";

/// Same options the engine's own asset loader uses (`schema/ron_loader.rs`): IMPLICIT_SOME.
fn ron_opts() -> ron::Options {
    ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
}

fn real_prefab_catalog() -> PrefabCatalog {
    let s = std::fs::read_to_string(format!("{DEMO}/prefabs/prefabs.ron"))
        .expect("demo prefabs.ron readable");
    ron_opts().from_str(&s).expect("demo prefabs.ron parses")
}

fn real_asset_catalog() -> AssetCatalog {
    let s = std::fs::read_to_string(format!("{DEMO}/assets.ron"))
        .expect("demo assets.ron readable");
    ron_opts().from_str(&s).expect("demo assets.ron parses")
}

fn real_behavior(path: &str) -> StateMachineAsset {
    let s = std::fs::read_to_string(format!("{DEMO}/{path}"))
        .unwrap_or_else(|e| panic!("{path} readable: {e}"));
    ron_opts().from_str(&s).unwrap_or_else(|e| panic!("{path} parses: {e}"))
}

fn real_animation_policy(path: &str) -> AnimationPolicy {
    let s = std::fs::read_to_string(format!("{DEMO}/{path}"))
        .unwrap_or_else(|e| panic!("{path} readable: {e}"));
    ron_opts().from_str(&s).unwrap_or_else(|e| panic!("{path} parses: {e}"))
}

/// The REAL `player_warrior` input map from the demo's prefabs.ron — it declares an `inputs:`
/// block but no `interact:` key, so `interact` falls back to `default_interact_key()` = "KeyF".
fn test_input_map() -> InputMap {
    let catalog = real_prefab_catalog();
    let def = catalog.prefabs.get("player_warrior").expect("player_warrior prefab present");
    def.components.inputs.clone().expect("player_warrior declares inputs")
}

fn test_controller() -> CharacterController {
    CharacterController {
        walk_speed: 5.0,
        run_speed: 8.0,
        rot_speed: 2.0,
        inputs: test_input_map(),
        is_running: false,
        jump_velocity: 5.94,
        double_jump_enabled: false,
        double_jump_velocity: 5.94,
        jumps_used: 0,
        max_jumps: 1,
        collider_radius: 0.4,
        ground_cast_length: 0.3,
        max_walkable_slope_deg: 45.0,
        coyote_time_secs: 0.1,
        coyote_ticks_remaining: 0,
        idle_drag: 0.8,
        jump_air_grace: 0,
        jump_liftoff_y: None,
    }
}

/// Local mirror of `message_interpreter::rewrite_self` (that function is `pub(crate)`, not
/// reachable from this external test crate) — covers just the `Action` variants used by
/// `enemy_zombie.behavior.ron` and `lootable_corpse.behavior.ron`'s entry actions. Needed because
/// `resolve_pending_behaviors_system` (the real production path) fires the initial state's
/// `entry_actions` with this exact substitution once a `PendingBehavior` asset loads — this test
/// file spawns entities directly instead of going through that system, so it must replicate the
/// same behavior manually or timers those entry actions arm (e.g. the corpse's 600s ambient
/// decay) would never get armed, which is a test-fidelity gap, not a real one.
fn self_sub(action: Action, id: &str) -> Action {
    match action {
        Action::EmitEvent(e) => Action::EmitEvent(e.replace("{self}", id)),
        Action::EmitEventAfterDelay { event, delay_secs } =>
            Action::EmitEventAfterDelay { event: event.replace("{self}", id), delay_secs },
        Action::PlayAnimationOn { target, clip, start_at_fraction, freeze } =>
            Action::PlayAnimationOn { target: target.replace("{self}", id), clip, start_at_fraction, freeze },
        Action::SpawnEffect { key, position, entity } =>
            Action::SpawnEffect { key, position, entity: entity.map(|e| e.replace("{self}", id)) },
        Action::ShowFloatingText { entity, text, offset } =>
            Action::ShowFloatingText { entity: entity.replace("{self}", id), text: text.replace("{self}", id), offset },
        Action::SetEntityVisible { entity, visible } =>
            Action::SetEntityVisible { entity: entity.replace("{self}", id), visible },
        Action::SetStat { key, value } => Action::SetStat { key: key.replace("{self}", id), value },
        Action::ResetToSpawn(t) => Action::ResetToSpawn(t.replace("{self}", id)),
        Action::Despawn(t) => Action::Despawn(t.replace("{self}", id)),
        Action::SetDespawnTimer { entity, delay_secs } =>
            Action::SetDespawnTimer { entity: entity.replace("{self}", id), delay_secs },
        Action::AddItem { entity, item_key, count } =>
            Action::AddItem { entity: entity.replace("{self}", id), item_key, count },
        Action::RemoveItem { entity, item_key, count } =>
            Action::RemoveItem { entity: entity.replace("{self}", id), item_key, count },
        Action::OpenContainer(t) => Action::OpenContainer(t.replace("{self}", id)),
        Action::Spawn { prefab, id: spawn_id, position, spawn_point, yaw_deg, at_entity } =>
            Action::Spawn {
                prefab,
                id: spawn_id.map(|i| i.replace("{self}", id)),
                position, spawn_point, yaw_deg,
                at_entity: at_entity.map(|e| e.replace("{self}", id)),
            },
        other => other,
    }
}

/// Queues `fsm`'s `initial`-state entry actions for `id`, matching what
/// `resolve_pending_behaviors_system` does the moment a real `PendingBehavior` asset loads.
fn fire_initial_entry_actions(app: &mut App, fsm: &StateMachineAsset, initial: &str, id: &str) {
    if let Some(state_def) = fsm.states.iter().find(|s| s.name == initial) {
        let mut queue = app.world_mut().resource_mut::<ActionQueue>();
        for action in &state_def.entry_actions {
            queue.push(self_sub(action.clone(), id));
        }
    }
}

/// Inserts the real demo's `LoadedPrefabCatalog`/`LoadedAssetCatalog` — needed for any test that
/// exercises a real `Action::Spawn` (the corpse swap), since the executor looks prefab/model keys
/// up in these before queueing. GLB paths are never actually fetched in this test harness (no
/// `AssetServer` file I/O happens synchronously) — `spawn_tests.rs` establishes this same pattern.
fn insert_real_catalogs(app: &mut App) {
    app.world_mut().insert_resource(LoadedPrefabCatalog(real_prefab_catalog()));
    app.world_mut().insert_resource(LoadedAssetCatalog(real_asset_catalog()));
}

/// Spawns a monster entity driven by its own real `.behavior.ron` (health + FSM only — as of v2,
/// monsters no longer carry `interactable`/`inventory` directly, so this only builds what
/// `attach_prefab_features` would for the `stat_templates`/`behavior` fields).
fn spawn_real_monster(app: &mut App, prefab_key: &str, behavior_path: &str, id: &str, pos: Vec3) -> Entity {
    let catalog = real_prefab_catalog();
    let def = catalog.prefabs.get(prefab_key).unwrap_or_else(|| panic!("{prefab_key} prefab present"));

    let mut stat_map = StatMap::default();
    for tpl in &def.stat_templates {
        stat_map.0.insert(
            tpl.key.clone(),
            LiveStat::new(StatDef {
                base: tpl.base,
                min: tpl.min,
                max: tpl.max,
                soft_max: None,
                regen_rate: tpl.regen_rate,
                regen_delay: tpl.regen_delay,
                thresholds: tpl
                    .thresholds
                    .iter()
                    .map(|t| ironhold_core::schema::stats::StatThreshold {
                        when: t.when.clone(),
                        emit: t.emit.replace("{self}", id),
                    })
                    .collect(),
            }),
        );
    }

    let fsm = real_behavior(behavior_path);
    let initial = fsm.initial_state.clone();
    let handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm);

    let e = app
        .world_mut()
        .spawn((
            SpawnId(id.to_string()),
            Transform::from_translation(pos),
            Visibility::Visible,
            stat_map,
            BehaviorHandle(handle),
            EntityFsmState { current: initial },
        ))
        .id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert(id.to_string(), e);
    e
}

/// Spawns a corpse entity directly (bypassing the monster-death swap) — mirrors
/// `attach_prefab_features` for the fields a `{monster}_corpse` prefab declares, using the real
/// shared `lootable_corpse.behavior.ron`.
fn spawn_real_corpse(app: &mut App, corpse_prefab_key: &str, id: &str, pos: Vec3) -> Entity {
    let catalog = real_prefab_catalog();
    let def = catalog.prefabs.get(corpse_prefab_key)
        .unwrap_or_else(|| panic!("{corpse_prefab_key} prefab present"));
    let interactable_def = def.interactable.as_ref()
        .unwrap_or_else(|| panic!("{corpse_prefab_key} has interactable"));
    let inv_def = def.inventory.as_ref().unwrap_or_else(|| panic!("{corpse_prefab_key} has inventory"));

    let mut inv = Inventory::new(inv_def.max_slots.max(4));
    for entry in &inv_def.initial_items {
        add_to_slots(&mut inv.slots, inv.max_slots, &entry.item_key, entry.count, None);
    }

    let fsm = real_behavior("behaviors/lootable_corpse.behavior.ron");
    let initial = fsm.initial_state.clone();
    let handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm.clone());

    let e = app
        .world_mut()
        .spawn((
            SpawnId(id.to_string()),
            Transform::from_translation(pos),
            Visibility::Visible,
            Interactable {
                radius: interactable_def.radius,
                hint_text: interactable_def.hint_text.clone(),
            },
            inv,
            BehaviorHandle(handle),
            EntityFsmState { current: initial.clone() },
        ))
        .id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert(id.to_string(), e);

    // Real corpses declare their own (minimal, death-pose-only) `animation_policy` — see
    // corpse_policy_zombie.ron and dynamic_animation_control.md. Loaded synchronously and
    // inserted directly here (skipping the async PendingAnimationPolicy resolution production
    // uses) since this test harness never fetches real asset files over an AssetServer anyway
    // (see insert_real_catalogs' doc comment) — the policy's *content* is what a test needs to
    // exercise, not the loading mechanics, which are covered by scene_lifecycle_tests.rs instead.
    if let Some(policy_path) = &def.animation_policy {
        let policy = real_animation_policy(policy_path);
        // Mirrors animation_policy_loader_system::resolve_initial_override — a real corpse's
        // death pose is applied SYNCHRONOUSLY at policy-attach time (via corpse_policy_{monster}.
        // ron's `initial_override: "death"`), not by a PlayAnimationOn in the behavior file's
        // entry_actions (there isn't one — see lootable_corpse.behavior.ron's header comment for
        // why a behavior-file PlayAnimationOn couldn't close this race reliably).
        let (current, should_loop, pending_seek, active_override) =
            match resolve_initial_override(&policy, 0.0) {
                Some(resolved) => {
                    let current = resolved.clip.clone().unwrap_or_default();
                    let should_loop = resolved.looping;
                    let pending_seek = resolved.seek_fraction.is_some() || resolved.frozen;
                    (current, should_loop, pending_seek, resolved)
                }
                None => (policy.base.idle.clone(), true, false, ActiveOverride::default()),
            };

        app.world_mut().entity_mut(e).insert((
            AnimationPolicyComponent(policy),
            AnimationController {
                current,
                last_played: String::new(),
                gltf_path: String::new(),
                gltf_handle: Default::default(),
                source_handles: Vec::new(),
                node_indices: Default::default(),
                graph_initialized: false,
                transition_ms: 0,
                should_loop,
                last_player_entity: None,
                pending_seek,
                graph_handle: None,
                awaiting_reveal: false,
                awaiting_reveal_since: None,
            },
            LocomotionState::default(),
            AnimationRequests::default(),
            active_override,
        ));
    }

    // Fire "fresh"'s entry actions (arms the 600s ambient decay).
    fire_initial_entry_actions(app, &fsm, &initial, id);
    e
}

fn spawn_container_panel(app: &mut App) -> Entity {
    app.world_mut()
        .spawn((
            Node::default(),
            ContainerPanelMarker { columns: 3, rows: 2, font_size: 14.0 },
            Visibility::Hidden,
        ))
        .id()
}

fn spawn_test_player(app: &mut App, pos: Vec3) -> Entity {
    app.world_mut()
        .spawn((
            SpawnId("player_01".to_string()),
            Transform::from_translation(pos),
            test_controller(),
            ironhold_core::capabilities::player::PlayerTarget::default(),
        ))
        .id()
}

fn press_f(app: &mut App) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyF);
    app.update();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::KeyF);
}

fn kill(app: &mut App, id: &str) {
    app.world_mut()
        .resource_mut::<ActionQueue>()
        .push(Action::ModifyStat { key: format!("{id}.health"), delta: -500.0 });
    for _ in 0..6 {
        app.update();
    }
}

/// Advances virtual time by `secs`. `Time<Virtual>`'s default `max_delta` clamps any single
/// step to 250 ms, so this steps in 0.25 s increments — a larger ManualDuration is silently
/// truncated, which is why a naive 25x1s loop only advances 6.25 s.
fn advance(app: &mut App, secs: f32) {
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(250),
    ));
    let steps = (secs / 0.25).ceil() as usize;
    for _ in 0..steps {
        app.update();
    }
}

/// A real death-spawned corpse's id is `"{monster_id}_corpse_{new_id}"` — the `{new_id}` suffix
/// is a monotonic counter value, never predictable in a test, so lookups against a real corpse
/// spawn go by prefix scan instead of a literal key. Panics if the count of matching entries
/// doesn't exactly match `expected_count`, so a test asserting "exactly one corpse" can't
/// silently pass against zero or accidentally match a second, unrelated corpse.
fn corpses_by_prefix(app: &App, monster_id: &str, expected_count: usize) -> Vec<Entity> {
    let prefix = format!("{monster_id}_corpse_");
    let found: Vec<Entity> = app
        .world()
        .resource::<SpawnRegistry>()
        .entities
        .iter()
        .filter(|(id, _)| id.starts_with(&prefix))
        .map(|(_, e)| *e)
        .collect();
    assert_eq!(
        found.len(),
        expected_count,
        "expected exactly {expected_count} corpse(s) with id prefix {prefix:?}, found {}",
        found.len()
    );
    found
}

// ── Monster death → corpse swap (the mechanism `at_entity` exists for) ─────────────────────

#[test]
fn zombie_death_swaps_to_a_corpse_at_the_same_position_and_despawns_the_original() {
    let mut app = setup_test_app();
    insert_real_catalogs(&mut app);
    app.update();

    let death_pos = Vec3::new(3.0, 0.0, -4.0);
    spawn_real_monster(&mut app, "enemy_zombie", "behaviors/enemy_zombie.behavior.ron", "zombie_01", death_pos);

    kill(&mut app, "zombie_01");
    assert!(
        app.world().resource::<SpawnRegistry>().entities.contains_key("zombie_01"),
        "zombie_01 must still exist right after death — only the swap (after the death anim) despawns it"
    );

    // Death anim is 3.0s before the swap fires; the original then gets SetDespawnTimer(1.0s),
    // not an immediate Despawn (see the "seamless overlap" comment in
    // enemy_zombie.behavior.ron's swap_to_corpse handler) — go a little past both.
    advance(&mut app, 4.5);

    assert!(
        !app.world().resource::<SpawnRegistry>().entities.contains_key("zombie_01"),
        "the original entity must be despawned once its corpse swap fires and its overlap timer elapses"
    );
    let corpse = corpses_by_prefix(&app, "zombie_01", 1)[0];

    let corpse_pos = app.world().get::<Transform>(corpse).unwrap().translation;
    assert_eq!(
        corpse_pos, death_pos,
        "at_entity must place the corpse at exactly the original entity's death position"
    );
    assert!(
        app.world().get::<Interactable>(corpse).is_some(),
        "the corpse must have come from the zombie_corpse prefab (carries Interactable, unlike the monster itself)"
    );
    assert!(
        app.world().get::<Inventory>(corpse).is_some(),
        "the corpse must have its own fresh Inventory from zombie_corpse's initial_items"
    );
}

/// Post-`corpse_new_id_retrofit` (2026-08-31): each corpse spawns under a fresh, unique
/// `"{self}_corpse_{new_id}"` id instead of the old fixed, reused `"{self}_corpse"` literal, so
/// there is no longer an id-collision guard to test — dying again before an earlier corpse
/// decays must let BOTH corpses coexist in the registry as two distinct, independently-alive
/// entities, not replace/orphan one for the other. See `planning/claude_suggestions.md`'s
/// resolved retrofit note for the superseded guard-based design this test used to cover.
#[test]
fn dying_again_before_the_old_corpse_decays_lets_both_corpses_coexist() {
    let mut app = setup_test_app();
    insert_real_catalogs(&mut app);
    app.update();

    spawn_real_monster(&mut app, "enemy_zombie", "behaviors/enemy_zombie.behavior.ron", "zombie_01", Vec3::ZERO);
    kill(&mut app, "zombie_01");
    advance(&mut app, 4.5);
    let first_corpse = corpses_by_prefix(&app, "zombie_01", 1)[0];

    // A second "zombie_01" (as the real respawn spawner would produce) dies again while the
    // first corpse is still alive and well within its own 600s decay window.
    spawn_real_monster(&mut app, "enemy_zombie", "behaviors/enemy_zombie.behavior.ron", "zombie_01", Vec3::new(1.0, 0.0, 1.0));
    kill(&mut app, "zombie_01");
    advance(&mut app, 4.5);

    let both = corpses_by_prefix(&app, "zombie_01", 2);
    assert!(
        both.contains(&first_corpse),
        "the first death's corpse must still be alive, not despawned by the second death"
    );
    let second_corpse = *both.iter().find(|e| **e != first_corpse).unwrap();
    assert!(app.world().get_entity(first_corpse).is_ok(), "first corpse must be a real, live entity");
    assert!(app.world().get_entity(second_corpse).is_ok(), "second corpse must be a real, live entity");
    assert_eq!(
        app.world().get::<Transform>(first_corpse).unwrap().translation,
        Vec3::ZERO,
        "the first corpse must keep the first death's position"
    );
    assert_eq!(
        app.world().get::<Transform>(second_corpse).unwrap().translation,
        Vec3::new(1.0, 0.0, 1.0),
        "the second corpse must have the second death's position"
    );
}

// ── Corpse loot/decay behavior (lootable_corpse.behavior.ron, shared by all three monsters) ──

#[test]
fn corpse_interact_opens_loot_panel() {
    let mut app = setup_test_app();
    app.update();
    let panel = spawn_container_panel(&mut app);
    spawn_test_player(&mut app, Vec3::ZERO);
    let corpse = spawn_real_corpse(&mut app, "zombie_corpse", "zombie_01_corpse", Vec3::ZERO);

    press_f(&mut app);
    app.update();

    assert_eq!(app.world().resource::<LoadedContainerUi>().active_container, Some(corpse));
    assert_eq!(*app.world().get::<Visibility>(panel).unwrap(), Visibility::Visible);
}

#[test]
fn looted_corpse_transitions_to_looted_and_decays_quickly() {
    let mut app = setup_test_app();
    app.update();
    spawn_container_panel(&mut app);
    spawn_test_player(&mut app, Vec3::ZERO);
    let corpse = spawn_real_corpse(&mut app, "zombie_corpse", "zombie_01_corpse", Vec3::ZERO);

    press_f(&mut app);
    app.update();
    app.world_mut().resource_mut::<ActionQueue>().push(Action::TakeAllFromContainer);
    app.update(); // action_executor runs TakeAllFromContainer, emits container.looted:{id}
    app.update(); // entity_fsm_interpreter_system reads that event (written after the interpreter
                  // chain ran the previous frame) and applies the "fresh" -> "looted" transition

    assert_eq!(
        app.world().get::<EntityFsmState>(corpse).unwrap().current, "looted",
        "container.looted:{{self}} must transition the corpse out of 'fresh'"
    );

    // "looted" arms a 20s decay — go a little past it.
    advance(&mut app, 20.5);
    assert!(
        app.world().get_entity(corpse).is_err(),
        "a looted corpse must despawn within its short decay window"
    );
}

#[test]
fn unlooted_corpse_decays_after_ten_minutes() {
    let mut app = setup_test_app();
    app.update();
    spawn_container_panel(&mut app);
    let corpse = spawn_real_corpse(&mut app, "snake_corpse", "snake_01_corpse", Vec3::ZERO);

    // Just under 10 minutes: still there.
    advance(&mut app, 599.0);
    assert!(app.world().get_entity(corpse).is_ok(), "an unlooted corpse must not decay before 10 minutes");

    advance(&mut app, 2.0);
    assert!(
        app.world().get_entity(corpse).is_err(),
        "an unlooted corpse must despawn once its 10-minute ambient decay elapses"
    );
}

#[test]
fn two_corpses_in_range_does_not_soft_lock_interact() {
    let mut app = setup_test_app();
    app.update();
    spawn_container_panel(&mut app);
    spawn_test_player(&mut app, Vec3::ZERO);
    spawn_real_corpse(&mut app, "zombie_corpse", "zombie_01_corpse", Vec3::ZERO);
    spawn_real_corpse(&mut app, "spider_corpse", "spider_01_corpse", Vec3::new(1.0, 0.0, 0.0));

    press_f(&mut app);
    app.update();
    assert!(app.world().resource::<LoadedContainerUi>().active_container.is_some(), "first F must open a panel");

    app.world_mut().resource_mut::<ActionQueue>().push(Action::CloseContainer);
    app.update();
    assert_eq!(
        app.world().resource::<LoadedInventoryUi>().panels_open, 0,
        "closing once must fully release the panel opened by two corpses in one interact press"
    );

    press_f(&mut app);
    app.update();
    assert!(
        app.world().resource::<LoadedContainerUi>().active_container.is_some(),
        "interact must still work after a two-corpse open/close cycle — this is the soft-lock the panels_open guard fixes"
    );
}

// ── Regressions found by the final mandatory review pass (debug-detective) ─────────────────

/// Finding 1 (historical — real corpses no longer reuse an id at all post-`corpse_new_id_retrofit`,
/// see the module-level "dying_again" test above; kept as a general invariant test since a
/// designer CAN still hand-author two entities sharing a literal id by mistake elsewhere): a
/// decay timer armed by an OLDER entity must never be able to affect a NEWER, unrelated entity
/// that happens to reuse the same id after the old one is despawned. `EmitEventAfterDelay` (a
/// global, string-matched event with no per-entry owner) could not guarantee this;
/// `SetDespawnTimer` (a component living directly on the entity) can, by construction — despawning
/// corpse A removes its `DespawnTimer` along with every other component it has, so there is no
/// timer left to mistakenly fire against corpse B later. This test manufactures the id collision
/// directly (`spawn_real_corpse` with a hand-picked literal id, not the real death→spawn path,
/// which no longer produces collisions on its own) purely to exercise that invariant.
#[test]
fn a_despawned_corpses_decay_timer_cannot_later_despawn_a_new_corpse_reusing_its_id() {
    let mut app = setup_test_app();
    app.update();
    spawn_container_panel(&mut app);

    // Corpse A: fresh 600s decay timer armed at spawn (t≈0).
    let corpse_a = spawn_real_corpse(&mut app, "zombie_corpse", "zombie_01_corpse", Vec3::ZERO);
    advance(&mut app, 100.0); // well within A's own 600s window — not decayed on its own yet

    // Manufacture an id collision directly (this manual Despawn stands in for whatever caused
    // corpse A to be replaced — no longer the monster's own death sequence, which never reuses a
    // corpse id post-retrofit; see the doc comment above).
    app.world_mut().resource_mut::<ActionQueue>().push(Action::Despawn("zombie_01_corpse".to_string()));
    app.update();
    assert!(app.world().get_entity(corpse_a).is_err(), "corpse A must be gone after the guard despawn");

    // Corpse B spawns fresh under the SAME reused id at t≈100, long before corpse A's ORIGINAL
    // would-be decay deadline (t≈600, since A was armed at t≈0) has elapsed.
    let corpse_b = spawn_real_corpse(&mut app, "zombie_corpse", "zombie_01_corpse", Vec3::new(1.0, 0.0, 1.0));

    // Advance to t≈650: well past corpse A's original t≈600 deadline (where a stale global event
    // would have fired and despawned whichever entity then held "zombie_01_corpse" — corpse B),
    // but still well before corpse B's OWN fresh deadline (t≈100+600=700).
    advance(&mut app, 550.0);

    assert!(
        app.world().get_entity(corpse_b).is_ok(),
        "corpse B must not be despawned by a timer that belonged to the earlier, already-despawned corpse A"
    );
}

/// Finding 3: despawning the entity behind the currently-open container panel (e.g. its own
/// decay firing, or any other Despawn) must tear the panel down the same way
/// `CloseContainer` does — otherwise the panel is left bound to a gone entity and `panels_open`
/// stuck above 0, permanently blocking interact/pickup/tab-targeting.
#[test]
fn despawning_the_currently_open_corpse_closes_its_container_panel() {
    let mut app = setup_test_app();
    app.update();
    let panel = spawn_container_panel(&mut app);
    spawn_test_player(&mut app, Vec3::ZERO);
    let corpse = spawn_real_corpse(&mut app, "zombie_corpse", "zombie_01_corpse", Vec3::ZERO);

    press_f(&mut app);
    app.update();
    assert_eq!(app.world().resource::<LoadedContainerUi>().active_container, Some(corpse));
    assert_eq!(app.world().resource::<LoadedInventoryUi>().panels_open, 1);

    app.world_mut().resource_mut::<ActionQueue>().push(Action::Despawn("zombie_01_corpse".to_string()));
    app.update();

    assert!(
        app.world().resource::<LoadedContainerUi>().active_container.is_none(),
        "despawning the open container must clear active_container"
    );
    assert_eq!(
        app.world().resource::<LoadedInventoryUi>().panels_open, 0,
        "despawning the open container must release panels_open, or interact/pickup/tab-targeting stays permanently blocked"
    );
    assert_eq!(
        *app.world().get::<Visibility>(panel).unwrap(), Visibility::Hidden,
        "the panel must be hidden once its container is gone"
    );
}

/// Real playtest regression (2026-08-26, post-fix verification): `despawn_timer_system` used to
/// despawn its entity directly via `Commands`, bypassing `Action::Despawn`'s own registry-removal
/// and container-close teardown entirely — so a corpse's *own* natural ambient decay (not a
/// manually pushed `Despawn`) never triggered the Finding 2/3 fixes above, even though the
/// dedicated tests for those fixes (which push `Action::Despawn` directly) passed. This is exactly
/// the gap those tests missed: they proved the teardown logic works when reached, not that every
/// real despawn path reaches it. `despawn_timer_system` now queues an `Action::Despawn` instead of
/// despawning directly, so it goes through the exact same teardown as any other despawn.
#[test]
fn a_corpses_own_decay_timer_closes_its_open_container_panel_and_clears_its_target() {
    let mut app = setup_test_app();
    app.update();
    let panel = spawn_container_panel(&mut app);
    let player = spawn_test_player(&mut app, Vec3::ZERO);
    let corpse = spawn_real_corpse(&mut app, "zombie_corpse", "zombie_01_corpse", Vec3::ZERO);

    press_f(&mut app);
    app.update();
    assert_eq!(app.world().resource::<LoadedContainerUi>().active_container, Some(corpse));

    // Target the open corpse directly (bypassing the click-to-target flow, not under test here).
    app.world_mut().get_mut::<PlayerTarget>(player).unwrap().0 = Some("zombie_01_corpse".to_string());
    app.world_mut().resource_mut::<CurrentTarget>().0 = Some("zombie_01_corpse".to_string());
    app.update();

    // Let the corpse's own 600s ambient decay fire naturally (SetDespawnTimer), NOT a manually
    // pushed Action::Despawn.
    advance(&mut app, 601.0);

    assert!(app.world().get_entity(corpse).is_err(), "the corpse must be gone after its ambient decay");
    assert!(
        !app.world().resource::<SpawnRegistry>().entities.contains_key("zombie_01_corpse"),
        "the decayed corpse's id must be removed from SpawnRegistry, or every downstream system \
         that looks it up (container-close, target-clear) never runs"
    );
    assert!(
        app.world().resource::<LoadedContainerUi>().active_container.is_none(),
        "the container panel must close when its own bound corpse decays out from under it"
    );
    assert_eq!(app.world().resource::<LoadedInventoryUi>().panels_open, 0);
    assert_eq!(*app.world().get::<Visibility>(panel).unwrap(), Visibility::Hidden);
    assert_eq!(
        app.world().get::<PlayerTarget>(player).unwrap().0, None,
        "the player's target on the decayed corpse must clear too"
    );
    assert_eq!(app.world().resource::<CurrentTarget>().0, None);
}

/// Finding 2: `target_auto_clear_system` only checked `Visibility::Hidden` on an entity still in
/// `SpawnRegistry` — a monster that's genuinely despawned (v2's death sequence, unlike v1's
/// hide-in-place revival) fell through untouched, so the player's stale target selection would
/// silently survive until the same id was reused by that slot's next respawn, up to 30s later.
#[test]
fn a_despawned_monsters_target_selection_clears_instead_of_surviving_to_the_next_respawn() {
    let mut app = setup_test_app();
    insert_real_catalogs(&mut app);
    app.update();

    spawn_real_monster(&mut app, "enemy_zombie", "behaviors/enemy_zombie.behavior.ron", "zombie_01", Vec3::ZERO);
    let player = spawn_test_player(&mut app, Vec3::ZERO);

    // Directly set the player's target to the live zombie (bypassing the click-to-target flow,
    // which isn't under test here) and mirror it into CurrentTarget the same way committing a
    // real target selection would.
    app.world_mut().get_mut::<PlayerTarget>(player).unwrap().0 = Some("zombie_01".to_string());
    app.world_mut().resource_mut::<CurrentTarget>().0 = Some("zombie_01".to_string());
    app.update();

    kill(&mut app, "zombie_01");
    advance(&mut app, 4.5); // past the death->corpse-swap despawn of the original entity

    assert_eq!(
        app.world().get::<PlayerTarget>(player).unwrap().0, None,
        "a genuinely despawned target must clear PlayerTarget, not silently persist"
    );
    assert_eq!(
        app.world().resource::<CurrentTarget>().0, None,
        "a genuinely despawned target must clear the global CurrentTarget mirror too"
    );
}

// ── Death pose at spawn (planning/features/dynamic_animation_control.md) ──────────────────

/// The bug this feature fixes: before it, `zombie_corpse`/`snake_corpse`/`spider_corpse` had no
/// `animation_policy` at all, so a corpse rendered in raw bind/rest pose, not lying dead. Fixed
/// by giving each corpse its own minimal `corpse_policy_{monster}.ron` with
/// `initial_override: "death"` (`start_at_fraction: 1.0, freeze: true` on the override itself).
///
/// **Not** a `PlayAnimationOn` in `lootable_corpse.behavior.ron`'s entry_actions — an earlier
/// version of this fix used that, and a real playtest found it wasn't actually race-free: the
/// behavior-file path (asset load -> entry_actions -> ActionQueue -> action_executor_system ->
/// AnimationRequests -> next resolver tick) is slower than the animation policy's own load path,
/// so the corpse could still flash through several frames of "death" playing unseeked and
/// looping (visibly: fell, snapped back to standing as the untended loop wrapped, fell again)
/// before the request won. `initial_override` applies synchronously the instant the policy loads
/// instead, closing the window entirely — see `resolve_initial_override` and
/// `lootable_corpse.behavior.ron`'s header comment.
///
/// This test cannot drive the real Bevy `AnimationPlayer` end-to-end — this harness never loads
/// a real GLTF asset (see `insert_real_catalogs`' doc comment), so `AnimationController.
/// graph_initialized` never becomes true here. What it CAN and does prove: the real RON wiring
/// (prefab -> corpse_policy_zombie.ron's `initial_override`) resolves into an `ActiveOverride`
/// that unambiguously requests the death pose, frozen, at the end of the clip, from the moment
/// the entity exists — not eventually, once some other request arrives. The resolver-level
/// mechanics of applying that (seek/freeze/pause) are covered by `animation_seek_freeze_tests.rs`
/// against synthetic Gltf/AnimationClip assets instead.
#[test]
fn a_freshly_spawned_corpse_immediately_requests_its_frozen_death_pose() {
    let mut app = setup_test_app();
    insert_real_catalogs(&mut app);
    app.update();

    spawn_real_corpse(&mut app, "zombie_corpse", "zombie_01_corpse", Vec3::ZERO);

    let entity = *app.world().resource::<SpawnRegistry>().entities.get("zombie_01_corpse")
        .expect("corpse must be registered");

    let assert_correct_pose = |app: &App, msg: &str| {
        let active = app.world().get::<ActiveOverride>(entity).expect("corpse must carry ActiveOverride");
        assert_eq!(active.clip.as_deref(), Some("Death01"), "{msg}: must resolve to the zombie's real death clip via the override-id lookup, not fall back to idle");
        assert_eq!(active.seek_fraction, Some(1.0), "{msg}: must request the final frame of the clip, not replay it from the start");
        assert!(active.frozen, "{msg}: must freeze at that frame instead of continuing to play (or idling once a non-looping clip would otherwise finish)");
        assert!(!active.looping, "{msg}: freeze must force looping off, matching the resolver's documented precedence");
    };

    // Correct immediately, with zero updates — the whole point of `initial_override` is that
    // there is no window where this could be anything else (unlike the earlier, reverted design,
    // where a PlayAnimationOn in the behavior file needed 1-2 updates to round-trip through the
    // action pipeline, and a real playtest found that window visibly leaked).
    assert_correct_pose(&app, "immediately after spawn_real_corpse");

    // Still correct after a couple of real resolver/playback ticks — proves the synchronously-
    // applied pose isn't disturbed by the normal per-frame resolver pass finding an empty
    // AnimationRequests queue (which would otherwise fall through to the base.idle fallback).
    app.update();
    app.update();
    assert_correct_pose(&app, "after 2 further updates");
}
