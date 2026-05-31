use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use notify::{Event, EventKind, RecursiveMode, Watcher};

use super::validate;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn utc_hms() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    format!("{h:02}:{m:02}:{s:02}")
}

fn ron_paths(event: &Event, project_dir: &Path) -> Vec<String> {
    event
        .paths
        .iter()
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
        .filter_map(|p| p.strip_prefix(project_dir).ok())
        .map(|p| p.display().to_string())
        .collect()
}

// ── Output ────────────────────────────────────────────────────────────────────

fn print_check(project_dir: &Path, changed: &[String]) {
    let result = validate::validate_project(project_dir);
    let time = utc_hms();

    if changed.is_empty() {
        print!("[{time}] initial check  →  ");
    } else {
        let label = changed.join(", ");
        println!("[{time}] {label}");
        print!("           →  ");
    }

    if result.all_valid {
        println!("OK ({} files)", result.file_count);
    } else {
        let n = result.errors.len();
        println!("ERROR ({n} issue{})", if n == 1 { "" } else { "s" });
        for err in &result.errors {
            println!("  {err}");
        }
    }
    println!();
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run(project_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !project_dir.is_dir() {
        return Err(format!("{}: not a directory", project_dir.display()).into());
    }

    // notify always returns absolute paths in events; canonicalize once so
    // strip_prefix matches regardless of how the user invoked the command.
    let canonical = project_dir
        .canonicalize()
        .map_err(|e| format!("cannot resolve {}: {e}", project_dir.display()))?;
    let project_dir = canonical.as_path();

    println!("Watching {} — Ctrl+C to stop", project_dir.display());
    println!();

    print_check(project_dir, &[]);

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = notify::recommended_watcher(tx)?;
    watcher.watch(project_dir, RecursiveMode::Recursive)?;

    for raw in &rx {
        let event = match raw {
            Ok(e) => e,
            Err(e) => {
                eprintln!("watch error: {e}");
                continue;
            }
        };

        // Skip access events — only care about content changes
        if matches!(
            event.kind,
            EventKind::Access(_) | EventKind::Other | EventKind::Any
        ) {
            continue;
        }

        let mut changed: Vec<String> = ron_paths(&event, project_dir);
        if changed.is_empty() {
            continue;
        }

        // Batch rapid saves (e.g. editor write-then-rename)
        std::thread::sleep(Duration::from_millis(50));
        while let Ok(Ok(e)) = rx.try_recv() {
            changed.extend(ron_paths(&e, project_dir));
        }
        changed.sort();
        changed.dedup();

        print_check(project_dir, &changed);
    }

    Ok(())
}
