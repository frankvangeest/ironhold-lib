//! Regression coverage for a real playtest bug: the player played the "falling" animation while
//! standing on ordinary flat ground near a physics prop (`3rd_person_game_demo`, next to the
//! `loot_display` platform + chest). Root cause: the ground shape-cast in `player_movement_system`
//! (`capabilities/player.rs`) excluded only the player's own rigid body, not other colliders — so
//! it could sweep straight into a nearby prop's `trigger_zone` sensor (a ghost `Collider::ball` +
//! `Sensor`, see `entity_spawner.rs`'s `attach_prefab_features`). The cast ball starts *inside* a
//! large, nearby sensor sphere, returning a `time_of_impact == 0` penetrating hit whose radial EPA
//! normal is ~horizontal — unwalkable by construction, vetoing the real floor sitting right there.
//! Fixed by adding `.exclude_sensors()` to the ground cast's `QueryFilter`. A related, independent
//! bug found during the same investigation — a penetrating hit's normal is not always unit length,
//! biasing the angle computation toward 90° — is fixed by `normalize_or_zero()`-ing it first; see
//! `non_unit_penetrating_normal_would_bias_the_angle_check_without_normalizing` (a direct math test
//! of the formula, since the exact sensor-penetration case that first surfaced it is no longer
//! reachable in production once sensors are excluded from the cast).
//!
//! See `planning/features/uphill_jump_lock.md` for the full writeup. Style follows
//! `player_slope_jump_tests.rs`: real Rapier physics, `player_movement_system` driven directly via
//! `run_system_once`, one `step()` == one `FixedUpdate` tick.
//!
//! Also covers a second, later-discovered regression from the same slope-walkability gate: a solid
//! (non-sensor) prop/wall pressed directly against the player could win the ground cast over the
//! real floor beneath it (its penetrating `time_of_impact == 0` contact always beats the floor's
//! non-zero toi), silently disabling jump entirely for any project with `double_jump_enabled:
//! false` (every shipped project's default). Fixed by re-querying, excluding any hit whose contact
//! point isn't actually underfoot, until a genuine floor candidate is found — see the
//! `solid_prop_taller_than_cast_ball_centre_no_longer_vetoes_when_pressed_against` test below and
//! `planning/backlog.md`'s former "solid prop disables jump" entry.
use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy_rapier3d::prelude::*;
use ironhold_core::runtime::{InputAction, InputActionMessage};
use ironhold_core::capabilities::player::{CharacterController, SpeedMultiplier, player_movement_system, ground_cast};
use ironhold_core::capabilities::animation_resolver::{LocomotionState, AnimationRequests};
use ironhold_core::schema::player::InputMap;

mod support;
use support::setup_test_app;

fn input_map() -> InputMap {
    InputMap {
        forward: "KeyW".to_string(),
        backward: "KeyS".to_string(),
        left: "KeyA".to_string(),
        right: "KeyD".to_string(),
        strafe_left: "KeyQ".to_string(),
        strafe_right: "KeyE".to_string(),
        jump: "Space".to_string(),
        run: "ShiftLeft".to_string(),
        interact: "KeyF".to_string(),
        strafe_mouse_button: Some("Left".to_string()),
        target_next: "Tab".to_string(),
        target_range: 30.0,
        gamepad_index: None,
        look_left: None, look_right: None, look_up: None, look_down: None,
        gamepad_jump: "South".to_string(),
        gamepad_run: "East".to_string(),
        gamepad_interact: "West".to_string(),
        gamepad_target_next: "North".to_string(),
        gamepad_deadzone: 0.15,
    }
}

struct Case {
    app: App,
    player: Entity,
}

/// What the ground shape-cast actually returned this tick.
#[derive(Debug, Clone)]
struct Probe {
    name: String,
    normal: Option<Vec3>,
}

impl Probe {
    fn angle_from_up_deg(&self) -> Option<f32> {
        self.normal.map(|n| n.normalize_or_zero().dot(Vec3::Y).clamp(-1.0, 1.0).acos().to_degrees())
    }
}

/// Calls the exact same `ground_cast()` `player_movement_system` calls (extracted to
/// `capabilities/player.rs` specifically so this probe can never silently drift from the real
/// ground check by hand-duplicating its cast/loop logic) — so whatever this reports is exactly
/// what the real ground check sees.
fn probe(case: &mut Case) -> Option<Probe> {
    case.app.world_mut().run_system_once(
        |players: Query<(Entity, &GlobalTransform, &CharacterController)>,
         names: Query<&Name>,
         ctx: ReadRapierContext| -> Option<Probe> {
            let ctx = ctx.single().ok()?;
            let (entity, gt, controller) = players.single().ok()?;
            let (hit_entity, hit) = ground_cast(&ctx, entity, gt.translation(), controller)?;
            Some(Probe {
                name: names.get(hit_entity).map(|n| n.to_string())
                    .unwrap_or_else(|_| format!("{hit_entity:?}")),
                normal: hit.details.map(|d| d.normal1),
            })
        }
    ).unwrap()
}

/// Flat solid ground whose top face is exactly y = 0 — stands in for `3rd_person_game_demo`'s
/// `ground_plane` primitive (a `Collider::cuboid`, not a `TriMesh`).
fn spawn_flat_ground(world: &mut World) {
    world.spawn((
        Name::new("ground"),
        RigidBody::Fixed,
        Collider::cuboid(200.0, 0.25, 200.0),
        Transform::from_xyz(0.0, -0.25, 0.0),
    ));
}

/// The `TriMesh` counterpart of `spawn_flat_ground` — this engine's real terrain collider family
/// (`capabilities/terrain.rs`'s `ComputedColliderShape::TriMesh`), a zero-thickness surface exactly
/// at y = 0, matching `player_slope_jump_tests.rs`'s `trimesh_ground_collider()` construction.
/// `tests/CLAUDE.md`'s "TriMesh vs Cuboid ground testing" rule requires this: the lifted-origin
/// ground cast has already broken twice on this geometry family specifically (see
/// `player.rs`'s ground-detection comment), so any test touching the same cast needs at least one
/// case against it, not just the convex-shape family that let both prior bugs ship undetected.
fn spawn_flat_trimesh_ground(world: &mut World) {
    let s = 400.0;
    let vertices = vec![
        Vec3::new(-s, 0.0, -s), Vec3::new(s, 0.0, -s), Vec3::new(s, 0.0, s), Vec3::new(-s, 0.0, s),
    ];
    let indices = vec![[0u32, 1, 2], [0, 2, 3]];
    world.spawn((
        Name::new("ground"),
        RigidBody::Fixed,
        Collider::trimesh(vertices, indices).expect("valid trimesh"),
        Transform::IDENTITY,
    ));
}

/// A replica of what `attach_prefab_features` builds for a prefab with `trigger_zone: (radius: r)`
/// — a child entity carrying `Collider::ball(r)` + `Sensor`, parented to the prop
/// (`runtime/scene_manager/entity_spawner.rs:90-101`).
fn spawn_trigger_zone_child(world: &mut World, parent: Entity, radius: f32) {
    let sensor = world.spawn((
        Name::new("prop/trigger_zone"),
        Collider::ball(radius),
        Sensor,
        ActiveEvents::COLLISION_EVENTS,
        Transform::default(),
    )).id();
    world.entity_mut(parent).add_child(sensor);
}

/// `chest_01`'s real collider pair from `3rd_person_game_demo/prefabs/prefabs.ron:390-391`
/// (RON `size` is full extents; `entity_spawner.rs:212-215` halves it for `Collider::cuboid`).
fn chest_collider() -> Collider {
    Collider::compound(vec![
        (Vec3::new(0.0, -0.125, 0.0), Quat::IDENTITY, Collider::cuboid(0.35, 0.275, 0.50)),
        (Vec3::new(0.0,  0.275, 0.0), Quat::IDENTITY, Collider::cuboid(0.34, 0.14,  0.49)),
    ])
}

/// Spawns the world: flat ground, then whatever props the test wants, then a player capsule
/// identical to `spawn_player_entity_core`'s construction, settled onto the ground at the origin.
fn setup(build_props: impl FnOnce(&mut World)) -> Case {
    setup_with_ground(spawn_flat_ground, build_props)
}

/// `setup`'s `TriMesh`-ground counterpart — see `spawn_flat_trimesh_ground`'s doc comment for why
/// this geometry family gets its own dedicated entry point rather than a boolean parameter.
fn setup_trimesh(build_props: impl FnOnce(&mut World)) -> Case {
    setup_with_ground(spawn_flat_trimesh_ground, build_props)
}

fn setup_with_ground(spawn_ground: fn(&mut World), build_props: impl FnOnce(&mut World)) -> Case {
    let mut app = setup_test_app();
    app.insert_resource(TimestepMode::Fixed { dt: 1.0 / 64.0, substeps: 1 });
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::ZERO));
    app.update();

    spawn_ground(app.world_mut());
    build_props(app.world_mut());

    let player = app.world_mut().spawn((
        Name::new("Player"),
        Transform::from_xyz(0.0, 0.02, 0.0),
        CharacterController {
            walk_speed: 5.0,
            run_speed: 10.0,
            rot_speed: 3.0,
            inputs: input_map(),
            is_running: false,
            jump_velocity: 6.0,
            double_jump_enabled: false,
            double_jump_velocity: 6.0,
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
        },
        LocomotionState::default(),
        AnimationRequests::default(),
        SpeedMultiplier(1.0),
        RigidBody::Dynamic,
        Collider::compound(vec![(
            Vec3::new(0.0, 0.9, 0.0),
            Quat::IDENTITY,
            Collider::capsule_y(0.5, 0.4),
        )]),
        LockedAxes::ROTATION_LOCKED,
        Damping { linear_damping: 0.5, angular_damping: 0.5 },
        Velocity::zero(),
        ExternalImpulse::default(),
        Friction { coefficient: 0.15, combine_rule: CoefficientCombineRule::Min },
    )).id();

    for _ in 0..40 { app.update(); }
    Case { app, player }
}

/// One `FixedUpdate` tick. `moving` walks the player toward +X (matching
/// `player_slope_jump_tests.rs`'s `step`). Returns this tick's `LocomotionState.is_grounded`.
fn step(case: &mut Case, moving: bool) -> bool {
    {
        let player = case.player;
        let mut msgs = case.app.world_mut().resource_mut::<Messages<InputActionMessage>>();
        msgs.clear();
        if moving {
            msgs.write(InputActionMessage { entity: player, action: InputAction::Move(Vec2::new(1.0, 0.0)) });
        }
    }
    case.app.world_mut().run_system_once(player_movement_system).unwrap();
    case.app.update();
    case.app.world().entity(case.player).get::<LocomotionState>().unwrap().is_grounded
}

fn player_pos(case: &Case) -> Vec3 {
    case.app.world().entity(case.player).get::<Transform>().unwrap().translation
}

/// Runs `ticks` idle ticks and returns the `is_grounded` reading on the final one.
fn settle_grounded(case: &mut Case, ticks: usize) -> bool {
    let mut last = true;
    for _ in 0..ticks { last = step(case, false); }
    last
}

#[test]
fn control_flat_ground_alone_stays_grounded() {
    let mut case = setup(|_| {});
    for tick in 0..30 {
        assert!(step(&mut case, false), "ungrounded on flat ground at tick {tick}");
    }
    let p = probe(&mut case).expect("flat ground must be detected");
    assert_eq!(p.name, "ground");
    assert!(p.angle_from_up_deg().unwrap() < 1.0, "flat ground normal must be ~vertical: {p:?}");
}

/// On a **penetrating** hit (`time_of_impact == 0`, `ShapeCastStatus::PenetratingOrWithinTargetDist`)
/// parry's EPA normal is not always unit length — measured `|n| ≈ 0.52` for a ball fully contained
/// in a small `trigger_zone` sensor during this bug's investigation (a case no longer reachable in
/// production now that sensors are excluded from the cast entirely, and not reliably reproducible
/// against other shape pairs, which is why this is a direct math test of `player.rs`'s formula
/// rather than a live-physics repro). Without `normalize_or_zero()`, the angle check computes
/// `acos(|n| * cos(theta))`, not the real angle theta — silently biasing every penetrating-hit
/// angle toward 90°. Harmless when the surface really is steep (still correctly unwalkable either
/// way), but would report a false "too steep" for a genuinely walkable penetrating contact.
#[test]
fn non_unit_penetrating_normal_would_bias_the_angle_check_without_normalizing() {
    // A genuinely walkable 20°-from-vertical surface normal, scaled to half length — reproducing
    // the *shape* of the magnitude anomaly actually measured during this bug's investigation
    // (`|n| ≈ 0.52` for a ball fully contained in a `trigger_zone` sensor), but picked specifically
    // to demonstrate real-world consequence rather than the exact measured value: that particular
    // vector happened to already be near-vertical, so its own bias was small. This one crosses the
    // 45° walkable threshold in the buggy direction, which is the actual failure mode the doc
    // comment above describes.
    let true_normal = Vec3::new(20f32.to_radians().sin(), 20f32.to_radians().cos(), 0.0);
    let raw_normal = true_normal * 0.5;
    assert!(
        (raw_normal.length() - 1.0).abs() > 0.01,
        "sanity: the fixture value itself must be non-unit, or this test proves nothing"
    );

    // `player.rs`'s actual formula, reproduced here: `normal.normalize_or_zero().dot(Vec3::Y)...`.
    let normalized_angle = raw_normal.normalize_or_zero().dot(Vec3::Y).clamp(-1.0, 1.0).acos().to_degrees();
    // What the pre-fix code computed: a bare `.dot()` on the raw, non-unit normal.
    let unnormalized_angle = raw_normal.dot(Vec3::Y).clamp(-1.0, 1.0).acos().to_degrees();

    println!("normalized={normalized_angle:.2} unnormalized={unnormalized_angle:.2}");
    assert!(
        (normalized_angle - 20.0).abs() < 0.1,
        "normalizing should recover the true 20° angle: got {normalized_angle:.2}"
    );
    assert!(
        unnormalized_angle > 45.0,
        "without normalizing, this genuinely-walkable 20° surface should misclassify as \
         unwalkable (>45°): got {unnormalized_angle:.2}"
    );

    // A fully degenerate (zero-length) normal must fall back to "unwalkable" (90°), matching the
    // existing "no computable normal" treatment for a `details: None` hit — not a NaN/panic from
    // dividing by zero.
    let degenerate_angle = Vec3::ZERO.normalize_or_zero().dot(Vec3::Y).clamp(-1.0, 1.0).acos().to_degrees();
    assert!((degenerate_angle - 90.0).abs() < 0.01, "zero-length normal must read as unwalkable (90°)");
}

// ---------------------------------------------------------------------------------------------
// The fixed bug: a `trigger_zone` sensor is now excluded from the ground cast entirely, so it can
// never veto a legitimate floor contact underneath/near it.
// ---------------------------------------------------------------------------------------------

#[test]
fn standing_near_a_trigger_zone_sensor_stays_grounded() {
    // A chest 1.5 m away, exactly as authored: solid compound collider + `trigger_zone: 2.5`. The
    // player never touches the chest's solid collider (1.5 m > 0.35 + 0.4) and is standing on the
    // flat ground plane, well within the sensor's un-fixed veto radius (~2.9 m).
    let mut case = setup(|world| {
        let chest = world.spawn((
            Name::new("chest_01"),
            RigidBody::Fixed,
            chest_collider(),
            Transform::from_xyz(1.5, 0.4, 0.0),
        )).id();
        spawn_trigger_zone_child(world, chest, 2.5);
    });

    for tick in 0..30 {
        assert!(step(&mut case, false), "sensor incorrectly vetoed the floor at tick {tick}");
    }
    let pos = player_pos(&case);
    assert!(pos.x.abs() < 0.05 && pos.y.abs() < 0.05, "player drifted: {pos:?}");

    let p = probe(&mut case).expect("the floor must still be detected with sensors excluded");
    assert_eq!(p.name, "ground", "the sensor must no longer win the cast");
    assert!(p.angle_from_up_deg().unwrap() < 1.0, "the floor's normal must read as walkable: {p:?}");
}

/// `TriMesh`-ground counterpart of the test above — required by `tests/CLAUDE.md`'s "TriMesh vs
/// Cuboid ground testing" rule, since the lifted-origin ground cast has already broken twice on
/// this exact geometry family (a zero-thickness surface has no "up" to resolve a penetrating hit
/// through cleanly). Confirms the sensor-exclusion fix holds on the engine's real terrain collider
/// type, not just the convex-shape family every other test in this file uses.
#[test]
fn standing_near_a_trigger_zone_sensor_stays_grounded_on_trimesh_terrain() {
    let mut case = setup_trimesh(|world| {
        let chest = world.spawn((
            Name::new("chest_01"),
            RigidBody::Fixed,
            chest_collider(),
            Transform::from_xyz(1.5, 0.4, 0.0),
        )).id();
        spawn_trigger_zone_child(world, chest, 2.5);
    });

    for tick in 0..30 {
        assert!(step(&mut case, false), "sensor incorrectly vetoed TriMesh ground at tick {tick}");
    }
    let p = probe(&mut case).expect("the TriMesh floor must still be detected with sensors excluded");
    assert_eq!(p.name, "ground", "the sensor must no longer win the cast");
    assert!(p.angle_from_up_deg().unwrap() < 1.0, "the floor's normal must read as walkable: {p:?}");
}

/// The organic version of the test above: the player *walks* across flat ground past a chest's
/// trigger zone without ever touching the chest itself. This is the exact playtest symptom.
#[test]
fn walking_past_a_trigger_zone_stays_grounded() {
    let mut case = setup(|world| {
        let chest = world.spawn((
            Name::new("chest_01"),
            RigidBody::Fixed,
            chest_collider(),
            Transform::from_xyz(6.0, 0.4, 0.0),
        )).id();
        spawn_trigger_zone_child(world, chest, 2.5);
    });

    for tick in 0..60 {
        let grounded = step(&mut case, true);
        assert!(grounded, "lost grounding at tick {tick}, x={:.3} — still short of the chest's \
                 solid collider (starts at x=5.65)", player_pos(&case).x);
    }
}

#[test]
fn sensor_never_vetoes_ground_at_any_distance() {
    // Sweep the same distances that used to straddle the (now-removed) veto radius
    // (`trigger_radius + collider_radius` ≈ 2.9 m for a 2.5 m sensor) — grounded at every one.
    // Starts at 1.0, not 0.5: `chest_collider()`'s own *solid* geometry (half-extent 0.35) plus the
    // player's `collider_radius` (0.4) already touch below ~0.75 m — any veto there would be the
    // separate, documented solid-prop limitation (see the tests further down), not a sensor
    // regression, so it's deliberately out of this sweep's range.
    for &dx in &[1.0_f32, 1.5, 2.0, 2.4, 2.6, 2.8, 2.9, 3.0, 3.5] {
        let mut case = setup(move |world| {
            let chest = world.spawn((
                Name::new("chest_01"),
                RigidBody::Fixed,
                chest_collider(),
                Transform::from_xyz(dx, 0.4, 0.0),
            )).id();
            spawn_trigger_zone_child(world, chest, 2.5);
        });
        let grounded = settle_grounded(&mut case, 20);
        assert!(grounded, "sensor at dx={dx} incorrectly vetoed the floor");
    }
}

#[test]
fn sensor_exclusion_is_independent_of_prop_rigid_body_kind() {
    for kind in ["none", "fixed", "dynamic"] {
        let mut case = setup(move |world| {
            let mut prop = world.spawn((
                Name::new("prop"),
                chest_collider(),
                Transform::from_xyz(1.5, 0.4, 0.0),
            ));
            match kind {
                "fixed" => { prop.insert(RigidBody::Fixed); }
                "dynamic" => { prop.insert((RigidBody::Dynamic, GravityScale(0.0), LockedAxes::all())); }
                _ => {}
            }
            let prop = prop.id();
            spawn_trigger_zone_child(world, prop, 2.5);
        });
        let grounded = settle_grounded(&mut case, 20);
        assert!(grounded, "sensor veto happened for prop rigid body kind = {kind}");
    }
}

#[test]
fn loot_display_replica_no_longer_reproduces_the_playtest_symptom() {
    // `loot_display` from prefabs.ron:502-533 placed so its 3x3 platform edge is 0.5 m from the
    // player: platform centre at x = 2.0 -> platform spans x 0.5..3.5. The chest child sits at the
    // platform centre, 2.0 m away, with `trigger_zone: 2.5` — this is the exact configuration from
    // the reported playtest screenshot.
    let mut case = setup(|world| {
        world.spawn((
            Name::new("loot_display/platform"),
            RigidBody::Fixed,
            Collider::cuboid(1.5, 0.1, 1.5),
            Transform::from_xyz(2.0, 0.1, 0.0),
        ));
        let chest = world.spawn((
            Name::new("loot_display/chest_01"),
            RigidBody::Fixed,
            chest_collider(),
            Transform::from_xyz(2.0, 0.6, 0.0),
        )).id();
        spawn_trigger_zone_child(world, chest, 2.5);
    });

    let grounded = settle_grounded(&mut case, 30);
    assert!(grounded, "playtest symptom reproduced: falling on flat ground beside the loot display");
}

// ---------------------------------------------------------------------------------------------
// Fixed by this change — previously a deliberately-deferred REGRESSION, not a pre-existing
// limitation: on `main`, the ground cast was proximity-only (`hit.is_some()`), so no collider's
// normal was ever load-bearing. `uphill_jump_lock.md`'s slope-walkability gate is what made a solid
// (non-sensor) prop/wall's normal matter at all, so a prop tall enough to reach the cast ball's
// centre vetoed the floor exactly like an unwalkable slope when the player stood pressed against
// it — and since `can_jump`'s only reachable branch requires `raw_grounded` (for every shipped
// project, which defaults `double_jump_enabled: false`), this **silently disabled jump entirely**,
// not just the animation. Fixed by re-querying the ground cast, excluding any hit whose contact
// point isn't actually underfoot (a side contact, not something the character is standing on),
// until a genuine floor candidate is found — see `player.rs`'s `raw_grounded` computation.
// ---------------------------------------------------------------------------------------------

#[test]
fn solid_prop_taller_than_cast_ball_centre_no_longer_vetoes_when_pressed_against() {
    let mut case = setup(|world| {
        world.spawn((
            Name::new("tall_box"),
            RigidBody::Fixed,
            Collider::cuboid(0.5, 0.5, 0.5),
            Transform::from_xyz(2.0, 0.5, 0.0),
        ));
    });
    for tick in 0..80 {
        assert!(step(&mut case, true), "tall wall incorrectly vetoed the floor at tick {tick}");
    }
    let p = probe(&mut case).expect("the floor must still be detected once the wall is excluded");
    assert_eq!(p.name, "ground", "the wall must no longer win the cast over the real floor");
    assert!(p.angle_from_up_deg().unwrap() < 1.0, "the floor's normal must read as walkable: {p:?}");

    // Prove the actual reported symptom is gone, not just the `is_grounded` proxy `step()` reads
    // above (coyote-buffered, so it would tolerate an occasional false `raw_grounded` reading):
    // pressing Jump while still pressed against the wall must actually launch the player.
    {
        let player = case.player;
        let mut msgs = case.app.world_mut().resource_mut::<Messages<InputActionMessage>>();
        msgs.clear();
        msgs.write(InputActionMessage { entity: player, action: InputAction::Move(Vec2::new(1.0, 0.0)) });
        msgs.write(InputActionMessage { entity: player, action: InputAction::Jump(true) });
    }
    case.app.world_mut().run_system_once(player_movement_system).unwrap();
    let controller = case.app.world().entity(case.player).get::<CharacterController>().unwrap();
    let velocity = case.app.world().entity(case.player).get::<Velocity>().unwrap();
    assert_eq!(controller.jumps_used, 1, "jump must actually fire while pressed against the wall");
    assert!(velocity.linvel.y > 0.0, "jump must impart upward velocity: {:?}", velocity.linvel);
}

/// `TriMesh`-ground counterpart of the test above — required by `tests/CLAUDE.md`'s "TriMesh vs
/// Cuboid ground testing" rule, since the lifted-origin ground cast has already broken twice on
/// this exact geometry family. Confirms the wall-exclusion fix holds against the engine's real
/// terrain collider type, not just the convex-shape family the test above uses.
#[test]
fn solid_prop_taller_than_cast_ball_centre_no_longer_vetoes_when_pressed_against_on_trimesh_terrain() {
    let mut case = setup_trimesh(|world| {
        world.spawn((
            Name::new("tall_box"),
            RigidBody::Fixed,
            Collider::cuboid(0.5, 0.5, 0.5),
            Transform::from_xyz(2.0, 0.5, 0.0),
        ));
    });
    for tick in 0..80 {
        assert!(step(&mut case, true), "tall wall incorrectly vetoed the TriMesh floor at tick {tick}");
    }
    let p = probe(&mut case).expect("the TriMesh floor must still be detected once the wall is excluded");
    assert_eq!(p.name, "ground", "the wall must no longer win the cast over the real floor");
    assert!(p.angle_from_up_deg().unwrap() < 1.0, "the floor's normal must read as walkable: {p:?}");
}

#[test]
fn solid_prop_shorter_than_cast_ball_centre_does_not_veto() {
    // The `loot_display` platform on its own: 0.2 m thick, so its side face tops out below the
    // cast ball's centre (feet + collider_radius + skin = 0.41 m) and the origin lift clears it.
    let mut case = setup(|world| {
        world.spawn((
            Name::new("low_platform"),
            RigidBody::Fixed,
            Collider::cuboid(1.5, 0.1, 1.5),
            Transform::from_xyz(2.0, 0.1, 0.0),
        ));
    });
    let mut ungrounded_ticks = 0;
    for _ in 0..80 {
        if !step(&mut case, true) { ungrounded_ticks += 1; }
    }
    assert_eq!(ungrounded_ticks, 0, "a prop shorter than the cast ball's centre must never veto");
}
