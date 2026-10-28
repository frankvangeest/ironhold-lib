//! Game namespacing. Several games share one Nakama, so every server-side id the adapter sends carries the game's id.
//! The types here can only be built through [`GameNs`], and the protocol functions accept nothing else: forgetting the
//! prefix is a compile error, not a data leak.

use ironhold_core::schema::backend::is_valid_backend_id;

/// Nakama accepts device ids of 10-128 bytes.
const DEVICE_ID_MIN: usize = 10;
const DEVICE_ID_MAX: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameNs(String);

/// A leaderboard id already prefixed with its game (`{game_id}.{name}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NsLeaderboardId(String);

impl NsLeaderboardId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl GameNs {
    pub fn new(game_id: &str) -> Result<Self, String> {
        if is_valid_backend_id(game_id) {
            Ok(Self(game_id.to_string()))
        } else {
            Err(format!("game_id {game_id:?} is not a valid id ([a-z0-9_], 1-32 characters)"))
        }
    }

    pub fn game_id(&self) -> &str {
        &self.0
    }

    /// `{game_id}.{name}`; `.` can never occur inside either part (ids are `[a-z0-9_]`), so two games cannot collide.
    pub fn leaderboard(&self, name: &str) -> Result<NsLeaderboardId, String> {
        if is_valid_backend_id(name) {
            Ok(NsLeaderboardId(format!("{}.{name}", self.0)))
        } else {
            Err(format!("leaderboard {name:?} is not a valid id"))
        }
    }

    /// `{game_id}:{uuid}`: a separate account per game by default.
    pub fn device_id(&self, uuid: &str) -> Result<String, String> {
        let id = format!("{}:{uuid}", self.0);
        if (DEVICE_ID_MIN..=DEVICE_ID_MAX).contains(&id.len()) && uuid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            Ok(id)
        } else {
            Err("the device id must be 10-128 characters of letters, digits and '-'".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_prefixed_and_separators_rejected() {
        let ns = GameNs::new("my_game").unwrap();
        assert_eq!(ns.leaderboard("best").unwrap().as_str(), "my_game.best");
        assert!(ns.leaderboard("b.est").is_err());
        assert!(GameNs::new("My Game").is_err());
        assert_eq!(ns.device_id("0123-abcd-4567").unwrap(), "my_game:0123-abcd-4567");
        assert!(ns.device_id("a:b").is_err());
        assert!(GameNs::new("a").unwrap().device_id("x").is_err());
    }
}
