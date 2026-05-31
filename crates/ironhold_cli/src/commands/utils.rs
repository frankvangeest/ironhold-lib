use std::path::{Path, PathBuf};

pub fn ron_from_str<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, ron::error::SpannedError> {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(s)
}

pub fn silent_parse<T: serde::de::DeserializeOwned>(project_dir: &Path, rel_path: &str) -> Option<T> {
    let full = project_dir.join(rel_path);
    if !full.exists() {
        return None;
    }
    let content = std::fs::read_to_string(full).ok()?;
    ron_from_str::<T>(&content).ok()
}

pub fn glob_dir(project_dir: &Path, subdir: &str, suffix: &str) -> Vec<PathBuf> {
    let dir = project_dir.join(subdir);
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.to_str().map(|s| s.ends_with(suffix)).unwrap_or(false))
        .collect();
    paths.sort();
    paths
}

pub fn rel(project_dir: &Path, full: &Path) -> String {
    full.strip_prefix(project_dir)
        .unwrap_or(full)
        .to_string_lossy()
        .replace('\\', "/")
}
