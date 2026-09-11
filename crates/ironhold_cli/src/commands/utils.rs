use std::path::{Path, PathBuf};

use ironhold_core::schema::ProjectConfig;

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

/// Finds the project's `.project.ron` file by name (whatever it's actually called — there's no
/// fixed convention filename, unlike every other config file this crate reads). Picks the FIRST
/// match in `read_dir` order when more than one exists in the same directory — inert for every
/// real project (`assets/projects/integration_tests/`'s three all declare identical catalog
/// paths, verified live) but no longer purely theoretical the moment one of those three configs
/// is ever edited to relocate a catalog independently of the others: `query`/`stats` now resolve
/// entirely different catalog data than intended, silently and deterministically-per-run, with
/// no signal at all (debug-detective finding, `feature/cli_query_stats_paths`'s review,
/// 2026-09-11). Warns to stderr rather than erroring — this function has no error-reporting
/// contract of its own and is called from read-only commands that shouldn't hard-fail over an
/// ambiguity that happens to be inert today.
pub fn find_project_ron(project_dir: &Path) -> Option<String> {
    let mut matches: Vec<String> = std::fs::read_dir(project_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".project.ron"))
        .collect();
    matches.sort();
    if matches.len() > 1 {
        eprintln!(
            "Warning: {} has {} *.project.ron files ({}) — using {:?}, the rest are ignored",
            project_dir.display(), matches.len(), matches.join(", "), matches[0]
        );
    }
    matches.into_iter().next()
}

/// `prefab_catalog`/`asset_catalog` resolved for `query`/`stats` specifically -- see
/// `resolve_catalog_paths`'s own doc comment for what "resolved" means here and why it's a
/// deliberate divergence from the runtime, not a general-purpose catalog-path resolver.
pub struct ResolvedCatalogPaths {
    pub prefab_catalog: String,
    pub asset_catalog: String,
}

/// Resolves `ProjectConfig.prefab_catalog`/`.asset_catalog` against their convention-path
/// fallbacks for `query`/`stats` (NOT `validate.rs`, which needs its own stricter
/// `load_configured_catalog` instead -- a configured-but-missing path there is a hard error with
/// a field-named message and a `path_case_mismatch` check, neither of which this function's
/// plain `unwrap_or` can express; the two resolvers are deliberately separate, not one shared by
/// three). `query.rs`/`stats.rs` previously hardcoded the convention-path literal outright and
/// either hard-errored or silently reported zero entries on a project that relocated a catalog,
/// even though `validate` on the same project passed clean (system-architect finding,
/// `feature/configurable_catalog_paths`'s review, 2026-09-04).
///
/// The convention-path fallback itself is a deliberate divergence from the runtime, which the
/// schema doc (`schema/project.rs`) is explicit loads NOTHING for an unset field, no guess at a
/// convention path at all -- `validate.rs` accepts this same divergence (and ships an
/// `unset_catalog_path_with_convention_file` `--strict` warning specifically to surface it) so a
/// project predating configurable paths, or a bare fixture with no `.project.ron`, still gets
/// checked/queried rather than silently skipped. `query`/`stats` inherit that same posture here,
/// for the same reason.
///
/// One `.project.ron` parse per call (not per catalog) -- callers needing both paths get them
/// from one `ResolvedCatalogPaths`, not two separate lookups.
pub fn resolve_catalog_paths(project_dir: &Path) -> ResolvedCatalogPaths {
    let config: Option<ProjectConfig> = find_project_ron(project_dir)
        .and_then(|name| silent_parse(project_dir, &name));
    // `Some("")` (an accidentally-emptied field) is treated the same as unset, not as a literal
    // empty relative path -- `project_dir.join("")` resolves to the project directory itself,
    // which `.exists()` (a directory does) but can't be read as a file, producing a confusing
    // "" not found" error with the field name silently dropped (debug-detective finding,
    // `feature/cli_query_stats_paths`'s review, 2026-09-11).
    ResolvedCatalogPaths {
        prefab_catalog: config.as_ref()
            .and_then(|c| c.prefab_catalog.clone())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "prefabs/prefabs.ron".to_string()),
        asset_catalog: config.as_ref()
            .and_then(|c| c.asset_catalog.clone())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "assets.ron".to_string()),
    }
}
