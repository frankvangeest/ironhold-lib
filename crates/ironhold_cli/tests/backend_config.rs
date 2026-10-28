//! `ironhold backend config`: the server-admin command that generates the Nakama module's config from `backend.ron` files.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn ironhold(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ironhold")).args(args).output().expect("failed to run ironhold")
}

fn demo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/projects/backend_demo")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ironhold_backend_config_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A minimal project folder whose `backend.ron` is the given text.
fn project_with(dir: &Path, project_id: &str, backend: Option<&str>) -> PathBuf {
    let p = dir.join(project_id);
    std::fs::create_dir_all(&p).unwrap();
    let backend_line = if backend.is_some() { "backend_path: \"backend.ron\"," } else { "" };
    std::fs::write(
        p.join(format!("{project_id}.project.ron")),
        format!(r#"ProjectConfig(schema_version: 3, project_id: "{project_id}", initial_scene: "scenes/main.scene.ron", {backend_line})"#),
    )
    .unwrap();
    if let Some(b) = backend {
        std::fs::write(p.join("backend.ron"), b).unwrap();
    }
    p
}

const GOOD: &str = r#"BackendConfig(schema_version: 1, endpoint: Some("http://localhost:7350"),
    leaderboards: [Leaderboard(name: "best", ranking: HigherIsBetter, keep: Best)])"#;

#[test]
fn prints_the_generated_config_for_the_demo() {
    let out = ironhold(&["backend", "config", demo().to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(r#"{ id = "backend_demo.best", sort = "desc", operator = "best" }"#), "{text}");
}

#[test]
fn out_writes_the_config_and_the_module_and_several_projects_become_one_server() {
    let dir = scratch("out");
    let a = project_with(&dir, "game_a", Some(GOOD));
    let b = project_with(&dir, "game_b", Some(GOOD));
    let out_dir = dir.join("modules");
    let out = ironhold(&["backend", "config", a.to_str().unwrap(), b.to_str().unwrap(), "--out", out_dir.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let games = std::fs::read_to_string(out_dir.join("ironhold_games.lua")).unwrap();
    assert!(games.contains("game_a.best") && games.contains("game_b.best"), "{games}");
    let module = std::fs::read_to_string(out_dir.join("ironhold.lua")).unwrap();
    assert!(module.contains("register_req_before"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_project_without_a_backend_an_invalid_one_and_a_duplicate_game_are_refused() {
    let dir = scratch("bad");
    let none = project_with(&dir, "plain", None);
    let out = ironhold(&["backend", "config", none.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("no backend_path"));

    let invalid = project_with(&dir, "invalid", Some(r#"BackendConfig(schema_version: 1, endpoint: Some("ftp://x"))"#));
    let out = ironhold(&["backend", "config", invalid.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("invalid"));

    // Same game_id through an explicit override.
    let a = project_with(&dir, "first", Some(&GOOD.replace("schema_version: 1,", "schema_version: 1, game_id: Some(\"shared\"),")));
    let b = project_with(&dir, "second", Some(&GOOD.replace("schema_version: 1,", "schema_version: 1, game_id: Some(\"shared\"),")));
    let out = ironhold(&["backend", "config", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("more than one project"));
    let _ = std::fs::remove_dir_all(&dir);
}
