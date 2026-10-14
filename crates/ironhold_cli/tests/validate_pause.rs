//! `ironhold validate --strict` checks for the "real pause" feature (`planning/features/real_pause.md`
//! section 7): `unrecoverable_pause`, `toggle_pause_in_entry_exit`, `pause_in_behavior`,
//! `pause_without_resume`, `delayed_event_while_paused`, `claimed_gamepad_collision`,
//! `overlay_scene_bindings_ignored`. Each has a true positive and a false positive. Projects are
//! generated into a temp dir per test (small RON snippets), rather than a fixture dir per case.

use std::path::PathBuf;
use std::process::Command;

struct Proj {
    dir: PathBuf,
}

impl Proj {
    /// `fsm_body` is spliced into the `StateMachineAsset` after `schema_version`/`initial_state`.
    fn new(name: &str, fsm_body: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("ironhold_validate_pause_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = Proj { dir };
        p.write(
            "p.project.ron",
            r#"(
    schema_version: 3,
    initial_scene: "scenes/main.scene.ron",
    state_machine_path: "logic/state_machine.ron",
)"#,
        );
        p.write("scenes/main.scene.ron", "(schema_version: 2, name: \"main\", entities: [])");
        p.write(
            "logic/state_machine.ron",
            &format!("(\n    schema_version: 1,\n    initial_state: \"playing\",\n{fsm_body}\n)"),
        );
        p
    }

    fn write(&self, rel: &str, content: &str) {
        let path = self.dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    /// Runs `ironhold --json validate --strict` (text mode does not print the warning code) and
    /// returns (exit code, `(type, message)` per strict warning, raw stdout).
    fn strict(&self) -> (i32, Vec<(String, String)>, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_ironhold"))
            .args(["--json", "validate", "--strict"])
            .arg(&self.dir)
            .output()
            .expect("failed to run ironhold");
        let raw = String::from_utf8_lossy(&out.stdout).into_owned();
        let json: serde_json::Value = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("validate --json output is not JSON ({e}):
{raw}"));
        let warnings = json["strict_warnings"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|w| {
                        (
                            w["type"].as_str().unwrap_or_default().to_string(),
                            w["message"].as_str().unwrap_or_default().to_string(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        (out.status.code().unwrap_or(-1), warnings, raw)
    }
}

impl Drop for Proj {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Asserts exit 1 and at least one strict warning of type `code`; returns those warnings' messages
/// joined by newlines.
fn assert_flags(p: &Proj, code: &str) -> String {
    let (exit, warnings, raw) = p.strict();
    assert_eq!(exit, 1, "expected exit 1 under --strict:
{raw}");
    let msgs: Vec<&str> = warnings.iter().filter(|(t, _)| t == code).map(|(_, m)| m.as_str()).collect();
    assert!(!msgs.is_empty(), "expected a `{code}` warning:
{raw}");
    msgs.join("
")
}

fn assert_clean_of(p: &Proj, code: &str) {
    let (exit, warnings, raw) = p.strict();
    assert_ne!(exit, 2, "tool error:
{raw}");
    assert!(!warnings.iter().any(|(t, _)| t == code), "did not expect `{code}`:
{raw}");
}

// ── unrecoverable_pause ───────────────────────────────────────────────────────

#[test]
fn unrecoverable_pause_flags_pause_without_resume() {
    let p = Proj::new(
        "unrec_tp",
        r#"states: [
        (name: "playing"),
        (name: "paused", entry_actions: [PauseGame]),
    ],"#,
    );
    let out = assert_flags(&p, "unrecoverable_pause");
    assert!(out.contains("state \"paused\" pauses the game"), "{out}");
    assert!(out.contains("ResumeGame"), "{out}");
}

#[test]
fn unrecoverable_pause_clean_with_resume_in_exit_actions() {
    let p = Proj::new(
        "unrec_pair",
        r#"states: [
        (name: "playing"),
        (name: "paused", entry_actions: [PauseGame], exit_actions: [ResumeGame]),
    ],"#,
    );
    assert_clean_of(&p, "unrecoverable_pause");
}

#[test]
fn unrecoverable_pause_clean_with_replace_load_scene_exit() {
    let p = Proj::new(
        "unrec_loadscene_exit",
        r#"states: [
        (name: "playing"),
        (name: "game_over", entry_actions: [PauseGame], exit_actions: [LoadScene("scenes/main.scene.ron")]),
    ],"#,
    );
    assert_clean_of(&p, "unrecoverable_pause");
}

#[test]
fn unrecoverable_pause_clean_with_replace_load_scene_in_on_binding() {
    let p = Proj::new(
        "unrec_loadscene_on",
        r#"states: [
        (name: "playing"),
        (name: "game_over", entry_actions: [PauseGame], on: [
            (event: "scene.ready:main", do_actions: [LoadScene("scenes/main.scene.ron")]),
        ]),
    ],"#,
    );
    assert_clean_of(&p, "unrecoverable_pause");
}

#[test]
fn unrecoverable_pause_resume_in_other_state_does_not_silence_it() {
    let p = Proj::new(
        "unrec_other_state",
        r#"states: [
        (name: "playing", exit_actions: [ResumeGame]),
        (name: "paused", entry_actions: [PauseGame]),
    ],"#,
    );
    assert_flags(&p, "unrecoverable_pause");
}

// ── toggle_pause_in_entry_exit ────────────────────────────────────────────────

#[test]
fn toggle_pause_in_entry_actions_flagged() {
    let p = Proj::new(
        "toggle_tp",
        r#"states: [
        (name: "playing"),
        (name: "paused", entry_actions: [TogglePause]),
    ],"#,
    );
    let out = assert_flags(&p, "toggle_pause_in_entry_exit");
    assert!(out.contains("TogglePause in entry_actions"), "{out}");
}

#[test]
fn toggle_pause_in_exit_actions_flagged() {
    let p = Proj::new(
        "toggle_exit",
        r#"states: [
        (name: "playing", exit_actions: [TogglePause]),
    ],"#,
    );
    assert_flags(&p, "toggle_pause_in_entry_exit");
}

#[test]
fn toggle_pause_in_on_binding_is_fine() {
    let p = Proj::new(
        "toggle_fp",
        r#"states: [(name: "playing")],
    global_on: [(event: "scene.ready:main", do_actions: [TogglePause])],"#,
    );
    assert_clean_of(&p, "toggle_pause_in_entry_exit");
}

// ── pause_in_behavior ─────────────────────────────────────────────────────────

const BEHAVIOR_PAUSE: &str = r#"(
    schema_version: 1,
    initial_state: "idle",
    states: [(name: "idle", on: [(event: "scene.ready:main", do_actions: [PauseGame])])],
)"#;

#[test]
fn pause_in_behavior_flagged() {
    let p = Proj::new("beh_tp", "states: [(name: \"playing\")],");
    p.write("behaviors/boss.behavior.ron", BEHAVIOR_PAUSE);
    let out = assert_flags(&p, "pause_in_behavior");
    assert!(out.contains("PauseGame"), "{out}");
}

#[test]
fn toggle_pause_in_behavior_flagged() {
    let p = Proj::new("beh_toggle", "states: [(name: \"playing\")],");
    p.write("behaviors/boss.behavior.ron", &BEHAVIOR_PAUSE.replace("PauseGame", "TogglePause"));
    assert_flags(&p, "pause_in_behavior");
}

#[test]
fn behavior_without_pause_is_clean() {
    let p = Proj::new("beh_fp", "states: [(name: \"playing\")],");
    p.write("behaviors/boss.behavior.ron", &BEHAVIOR_PAUSE.replace("PauseGame", "Log(\"hi\")"));
    assert_clean_of(&p, "pause_in_behavior");
}

// ── pause_without_resume ──────────────────────────────────────────────────────

#[test]
fn pause_without_resume_flagged() {
    let p = Proj::new(
        "pwr_tp",
        r#"states: [(name: "playing")],
    global_on: [(event: "scene.ready:main", do_actions: [PauseGame])],"#,
    );
    assert_flags(&p, "pause_without_resume");
}

#[test]
fn pause_without_resume_clean_when_resume_exists_elsewhere() {
    let p = Proj::new(
        "pwr_resume",
        r#"states: [(name: "playing", exit_actions: [ResumeGame])],
    global_on: [(event: "scene.ready:main", do_actions: [PauseGame])],"#,
    );
    assert_clean_of(&p, "pause_without_resume");
}

#[test]
fn pause_without_resume_clean_when_toggle_exists() {
    let p = Proj::new(
        "pwr_toggle",
        r#"states: [(name: "playing", on: [(event: "scene.ready:main", do_actions: [TogglePause])])],
    global_on: [(event: "scene.ready:main", do_actions: [PauseGame])],"#,
    );
    assert_clean_of(&p, "pause_without_resume");
}

// ── delayed_event_while_paused ────────────────────────────────────────────────

#[test]
fn delayed_event_with_pause_flagged() {
    let p = Proj::new(
        "delay_tp",
        r#"states: [
        (name: "playing"),
        (name: "paused", entry_actions: [PauseGame, EmitEventAfterDelay(event: "x", delay_secs: 1.0)], exit_actions: [ResumeGame]),
    ],"#,
    );
    assert_flags(&p, "delayed_event_while_paused");
}

#[test]
fn delayed_event_in_on_binding_with_pause_entry_flagged() {
    let p = Proj::new(
        "delay_on",
        r#"states: [
        (name: "playing"),
        (name: "paused", entry_actions: [PauseGame], exit_actions: [ResumeGame], on: [
            (event: "scene.ready:main", do_actions: [EmitEventAfterDelay(event: "x", delay_secs: 1.0)]),
        ]),
    ],"#,
    );
    assert_flags(&p, "delayed_event_while_paused");
}

#[test]
fn delayed_event_without_pause_is_clean() {
    let p = Proj::new(
        "delay_fp",
        r#"states: [
        (name: "playing", entry_actions: [EmitEventAfterDelay(event: "x", delay_secs: 1.0)]),
        (name: "paused", entry_actions: [PauseGame], exit_actions: [ResumeGame]),
    ],"#,
    );
    assert_clean_of(&p, "delayed_event_while_paused");
}

// ── claimed_gamepad_collision ─────────────────────────────────────────────────

fn claimed_project(name: &str, claimed_button: &str, slot_gamepad_key: Option<&str>) -> Proj {
    let p = Proj::new(name, "states: [(name: \"playing\")],");
    p.write(
        "p.project.ron",
        &format!(
            r#"(
    schema_version: 3,
    initial_scene: "scenes/main.scene.ron",
    state_machine_path: "logic/state_machine.ron",
    prefab_catalog: "prefabs/prefabs.ron",
    global_claimed_gamepad_bindings: {{ "{claimed_button}": "toggle_pause" }},
)"#
        ),
    );
    p.write(
        "prefabs/prefabs.ron",
        r#"(
    schema_version: 2,
    prefabs: {
        "hero": (
            kind: Primitive,
            shape: Cuboid,
            player_index: 0,
            components: (tags: ["player"]),
            primitive: (radius: 0.4, height: 1.8),
        ),
    },
)"#,
    );
    let ui = match slot_gamepad_key {
        Some(k) => format!(
            r#"ui: [ActionBar((id: "bar", slots: [(key: "KeyQ", gamepad_key: "{k}", do_actions: [Log("x")])]))],"#
        ),
        None => String::new(),
    };
    p.write(
        "scenes/main.scene.ron",
        &format!(
            r#"(schema_version: 2, name: "main", entities: [(id: "h", prefab: "hero", transform: (translation: (0.0, 0.0, 0.0)))], {ui})"#
        ),
    );
    p
}

#[test]
fn claimed_binding_colliding_with_default_player_button_flagged() {
    // Default gamepad_jump is "South".
    let p = claimed_project("claim_tp", "South", None);
    let out = assert_flags(&p, "claimed_gamepad_collision");
    assert!(out.contains("gamepad_jump"), "{out}");
}

#[test]
fn claimed_binding_colliding_with_action_slot_gamepad_key_flagged() {
    let p = claimed_project("claim_slot", "Start", Some("Start"));
    let out = assert_flags(&p, "claimed_gamepad_collision");
    assert!(out.contains("ActionBar"), "{out}");
}

#[test]
fn claimed_binding_on_unused_button_is_clean() {
    let p = claimed_project("claim_fp", "Start", Some("DPadUp"));
    assert_clean_of(&p, "claimed_gamepad_collision");
}

// ── overlay_scene_bindings_ignored ────────────────────────────────────────────

fn overlay_project(name: &str, fsm_actions: &str, overlay_bindings: &str) -> Proj {
    let p = Proj::new(
        name,
        &format!(
            "states: [(name: \"playing\", on: [(event: \"scene.ready:main\", do_actions: [{fsm_actions}])])],"
        ),
    );
    p.write(
        "scenes/pause.scene.ron",
        &format!("(schema_version: 2, name: \"pause\", entities: [], {overlay_bindings})"),
    );
    p
}

#[test]
fn overlay_only_scene_with_key_bindings_flagged() {
    let p = overlay_project(
        "ov_tp",
        r#"LoadSceneOverlay("scenes/pause.scene.ron")"#,
        r#"scene_key_bindings: { "KeyR": "resume" },"#,
    );
    let out = assert_flags(&p, "overlay_scene_bindings_ignored");
    assert!(out.contains("only ever loaded as an overlay"), "{out}");
}

#[test]
fn toggle_overlay_only_scene_with_gamepad_bindings_flagged() {
    let p = overlay_project(
        "ov_toggle",
        r#"ToggleOverlay("scenes/pause.scene.ron")"#,
        r#"scene_unclaimed_gamepad_bindings: { "Start": "resume" },"#,
    );
    assert_flags(&p, "overlay_scene_bindings_ignored");
}

#[test]
fn overlay_scene_also_loaded_as_base_is_clean() {
    let p = overlay_project(
        "ov_base",
        r#"LoadSceneOverlay("scenes/pause.scene.ron"), LoadScene("scenes/pause.scene.ron")"#,
        r#"scene_key_bindings: { "KeyR": "resume" },"#,
    );
    assert_clean_of(&p, "overlay_scene_bindings_ignored");
}

#[test]
fn overlay_scene_without_bindings_is_clean() {
    let p = overlay_project("ov_none", r#"LoadSceneOverlay("scenes/pause.scene.ron")"#, "");
    assert_clean_of(&p, "overlay_scene_bindings_ignored");
}

// ── unrecoverable_pause: transitions leaving the state ────────────────────────

#[test]
fn resume_transition_plus_load_scene_button_without_resume_game_flagged() {
    let p = Proj::new(
        "unrec_transition",
        r#"states: [
        (name: "playing"),
        (name: "paused", entry_actions: [PauseGame], on: [
            (event: "ui.button_pressed:main_menu", do_actions: [LoadScene("scenes/main.scene.ron")]),
        ]),
    ],
    transitions: [(from: Some("paused"), on: "ui.button_pressed:resume", to: "playing")],"#,
    );
    let out = assert_flags(&p, "unrecoverable_pause");
    assert!(out.contains("ui.button_pressed:resume"), "{out}");
}

#[test]
fn any_state_transition_leaving_pause_flagged() {
    let p = Proj::new(
        "unrec_any_state",
        r#"states: [
        (name: "playing"),
        (name: "paused", entry_actions: [PauseGame]),
    ],
    transitions: [(on: "ui.button_pressed:resume", to: "playing")],"#,
    );
    assert_flags(&p, "unrecoverable_pause");
}

#[test]
fn resume_transition_with_resume_game_exit_is_clean() {
    let p = Proj::new(
        "unrec_transition_ok",
        r#"states: [
        (name: "playing"),
        (name: "paused", entry_actions: [PauseGame], exit_actions: [ResumeGame], on: [
            (event: "ui.button_pressed:main_menu", do_actions: [LoadScene("scenes/main.scene.ron")]),
        ]),
    ],
    transitions: [(from: Some("paused"), on: "ui.button_pressed:resume", to: "playing")],"#,
    );
    assert_clean_of(&p, "unrecoverable_pause");
}

#[test]
fn load_scene_with_only_scene_event_transitions_is_clean() {
    let p = Proj::new(
        "unrec_scene_only",
        r#"states: [
        (name: "playing"),
        (name: "game_over", entry_actions: [PauseGame], on: [
            (event: "ui.button_pressed:retry", do_actions: [LoadScene("scenes/main.scene.ron")]),
        ]),
    ],
    transitions: [(from: Some("game_over"), on: "scene.ready:main", to: "playing")],"#,
    );
    assert_clean_of(&p, "unrecoverable_pause");
}

#[test]
fn transition_from_other_state_does_not_count() {
    let p = Proj::new(
        "unrec_other_from",
        r#"states: [
        (name: "playing"),
        (name: "game_over", entry_actions: [PauseGame], on: [
            (event: "ui.button_pressed:retry", do_actions: [LoadScene("scenes/main.scene.ron")]),
        ]),
    ],
    transitions: [(from: Some("playing"), on: "ui.button_pressed:die", to: "game_over")],"#,
    );
    assert_clean_of(&p, "unrecoverable_pause");
}

#[test]
fn overlay_path_spelling_variants_still_match_base_load() {
    let p = overlay_project(
        "ov_norm",
        r#"LoadSceneOverlay("scenes/pause.scene.ron"), LoadScene("./scenes/pause.scene.ron")"#,
        r#"scene_key_bindings: { "KeyR": "resume" },"#,
    );
    assert_clean_of(&p, "overlay_scene_bindings_ignored");
}
