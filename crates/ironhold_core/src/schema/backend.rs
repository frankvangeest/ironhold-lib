//! `backend.ron`: which online features a project uses (`planning/features/backend_abstraction.md`).
//!
//! Referenced through `backend_path` in `.project.ron` (no convention-path fallback). A project without one runs fully
//! offline; the online actions then answer `backend.offline`.

use bevy::prelude::*;
use serde::Deserialize;

/// Highest `schema_version` this engine reads.
pub const BACKEND_SCHEMA_VERSION: u32 = 1;
/// A leaderboard list never holds more rows than this (the engine-written `leaderboard.<name>.<n>.*` variables are fixed-size).
pub const MAX_LEADERBOARD_ROWS: u32 = 10;
/// Longest `game_id`; ids are `[a-z0-9_]` so no separator used in storage keys can ever appear inside one.
pub const MAX_ID_LEN: usize = 32;

#[derive(Deserialize, Asset, TypePath, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct BackendConfig {
    pub schema_version: u32,
    /// Namespace on a shared server, `[a-z0-9_]{1,32}`. Defaults to the project's `project_id` when omitted.
    #[serde(default)]
    pub game_id: Option<String>,
    /// `None` = an offline project; `"fake://"` = the in-memory test backend; anything else is handed to the adapter the
    /// runner built in (`https://...`; plain `http` only for localhost/LAN).
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub features: Vec<BackendFeature>,
    #[serde(default)]
    pub leaderboards: Vec<LeaderboardDef>,
    /// Seconds a request may stay unanswered before the engine fails it (default 15, allowed 1-120).
    #[serde(default)]
    pub request_timeout_secs: Option<f32>,
}

/// Optional capabilities beyond sign-in and leaderboards (declared so a server can be provisioned for them).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendFeature {
    CloudSave,
    Lobbies,
    Matchmaker,
}

/// Written `Leaderboard(name: ..., ranking: ..., keep: ...)` in `backend.ron`.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields, rename = "Leaderboard")]
pub struct LeaderboardDef {
    pub name: String,
    #[serde(default)]
    pub ranking: Ranking,
    #[serde(default)]
    pub keep: Keep,
}

/// Which end of the score range is rank 1.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ranking {
    #[default]
    HigherIsBetter,
    LowerIsBetter,
}

/// What a player's second submission does to their first.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Keep {
    /// Only the best score counts (best according to `ranking`).
    #[default]
    Best,
    /// The newest submission replaces the old one.
    Latest,
    /// Submissions add up.
    Total,
}

/// `[a-z0-9_]{1,MAX_ID_LEN}`: the charset keeps `.`/`:` free as separators in namespaced keys.
pub fn is_valid_backend_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID_LEN && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// True for `endpoint` values that name the in-memory test backend.
pub fn is_fake_endpoint(endpoint: &str) -> bool {
    endpoint.starts_with("fake://")
}

/// What a designer can ask of the test backend through the endpoint: `fake://?latency=30&fail=submit,list`.
/// `latency` counts frames before an answer; `fail` is a comma list of `sign_in` (rejected), `unreachable`, `submit`, `list`
/// and `silent` (never answers, so the request times out).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FakeEndpointOptions {
    pub latency: Option<u64>,
    pub reject_sign_in: bool,
    pub unreachable: bool,
    pub submit: bool,
    pub list: bool,
    pub silent: bool,
}

/// Parses the query part of a `fake://` endpoint. An unknown option or value is an error (a typo must not silently do nothing).
pub fn parse_fake_endpoint(endpoint: &str) -> Result<FakeEndpointOptions, String> {
    let rest = endpoint.strip_prefix("fake://").ok_or_else(|| format!("{endpoint:?} is not a fake:// endpoint"))?;
    let mut out = FakeEndpointOptions::default();
    let Some(query) = rest.strip_prefix('?').or_else(|| rest.split_once('?').map(|(_, q)| q)) else {
        return if rest.is_empty() { Ok(out) } else { Err(format!("fake endpoint {endpoint:?}: expected fake:// or fake://?latency=N&fail=...")) };
    };
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "latency" => {
                out.latency = Some(value.parse().map_err(|_| format!("fake endpoint: latency {value:?} is not a whole number of frames"))?);
            }
            "fail" => {
                for f in value.split(',').filter(|f| !f.is_empty()) {
                    match f {
                        "sign_in" => out.reject_sign_in = true,
                        "unreachable" => out.unreachable = true,
                        "submit" => out.submit = true,
                        "list" => out.list = true,
                        "silent" => out.silent = true,
                        other => {
                            return Err(format!(
                                "fake endpoint: unknown failure {other:?} (use sign_in, unreachable, submit, list or silent)"
                            ))
                        }
                    }
                }
            }
            other => return Err(format!("fake endpoint: unknown option {other:?} (use latency or fail)")),
        }
    }
    Ok(out)
}

impl BackendConfig {
    /// Every problem the engine would trip over at runtime. `Ok` only when the list is empty.
    pub fn validate(&self) -> Result<(), String> {
        let errors = self.problems();
        if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
    }

    /// The same list as `validate`, one string per problem (the CLI reports them individually).
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.schema_version == 0 || self.schema_version > BACKEND_SCHEMA_VERSION {
            out.push(format!(
                "schema_version {} is not supported (this engine reads 1..={BACKEND_SCHEMA_VERSION})",
                self.schema_version
            ));
        }
        if let Some(id) = &self.game_id {
            if !is_valid_backend_id(id) {
                out.push(format!("game_id {id:?} must be 1-{MAX_ID_LEN} characters of [a-z0-9_]"));
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for lb in &self.leaderboards {
            if !is_valid_backend_id(&lb.name) {
                out.push(format!("leaderboard name {:?} must be 1-{MAX_ID_LEN} characters of [a-z0-9_]", lb.name));
            }
            if !seen.insert(lb.name.as_str()) {
                out.push(format!("leaderboard {:?} is declared twice", lb.name));
            }
        }
        if let Some(ep) = &self.endpoint {
            let ok = is_fake_endpoint(ep) || ep.starts_with("https://") || ep.starts_with("http://");
            if !ok {
                out.push(format!("endpoint {ep:?} must be \"fake://\", \"https://...\" or (local/LAN only) \"http://...\""));
            } else if is_fake_endpoint(ep) {
                if let Err(e) = parse_fake_endpoint(ep) {
                    out.push(e);
                }
            }
        }
        if let Some(t) = self.request_timeout_secs {
            if !(1.0..=120.0).contains(&t) {
                out.push(format!("request_timeout_secs {t} must be between 1 and 120"));
            }
        }
        out
    }

    /// The namespace this project uses on a server: the explicit `game_id`, else the project's `project_id`.
    pub fn effective_game_id(&self, project_id: Option<&str>) -> Option<String> {
        self.game_id.clone().or_else(|| project_id.map(str::to_string))
    }

    pub fn leaderboard(&self, name: &str) -> Option<&LeaderboardDef> {
        self.leaderboards.iter().find(|l| l.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> BackendConfig {
        ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
            .from_str(s)
            .unwrap()
    }

    #[test]
    fn the_documented_example_parses_and_validates() {
        let c = parse(
            r#"BackendConfig(
                schema_version: 1,
                game_id: "ironhold_demo",
                endpoint: "fake://",
                features: [CloudSave, Lobbies],
                leaderboards: [Leaderboard(name: "best_time", ranking: LowerIsBetter, keep: Best)],
            )"#,
        );
        assert_eq!(c.validate(), Ok(()));
        assert_eq!(c.leaderboard("best_time").unwrap().ranking, Ranking::LowerIsBetter);
        assert_eq!(c.features, vec![BackendFeature::CloudSave, BackendFeature::Lobbies]);
    }

    #[test]
    fn ids_with_separators_or_uppercase_are_rejected() {
        for bad in ["", "A", "a.b", "a:b", "a b", "ünï", &"x".repeat(MAX_ID_LEN + 1)] {
            assert!(!is_valid_backend_id(bad), "{bad:?} must be rejected");
        }
        assert!(is_valid_backend_id("ironhold_demo_2"));
    }

    #[test]
    fn duplicates_bad_versions_and_endpoints_are_reported() {
        let c = parse(
            r#"BackendConfig(
                schema_version: 2,
                game_id: "Bad.Id",
                endpoint: "ftp://x",
                leaderboards: [Leaderboard(name: "a"), Leaderboard(name: "a")],
            )"#,
        );
        let p = c.problems();
        assert_eq!(p.len(), 4, "{p:?}");
    }

    #[test]
    fn fake_endpoint_options_parse_and_typos_are_errors() {
        assert_eq!(parse_fake_endpoint("fake://"), Ok(FakeEndpointOptions::default()));
        let o = parse_fake_endpoint("fake://?latency=30&fail=submit,silent").unwrap();
        assert_eq!((o.latency, o.submit, o.silent, o.list), (Some(30), true, true, false));
        for bad in ["fake://?latency=x", "fake://?fail=nope", "fake://?speed=3", "fake://x"] {
            assert!(parse_fake_endpoint(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_effective_game_id_falls_back_to_the_project_id() {
        let c: BackendConfig = ron::from_str("BackendConfig(schema_version: 1)").unwrap();
        assert_eq!(c.effective_game_id(Some("proj")).as_deref(), Some("proj"));
        assert_eq!(c.effective_game_id(None), None);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let r: Result<BackendConfig, _> = ron::from_str("BackendConfig(schema_version: 1, nope: 3)");
        assert!(r.is_err());
    }
}
