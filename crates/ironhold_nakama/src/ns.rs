//! Game namespacing. Several games share one Nakama, so every server-side id the adapter sends carries the game's id.
//! The types here can only be built through [`GameNs`], and the protocol functions accept nothing else: forgetting the
//! prefix is a compile error, not a data leak.

use ironhold_core::schema::backend::is_valid_backend_id;

/// The stored device secret: 32 lowercase hex characters (128 bits from the OS random source). The runners and
/// [`GameNs::device_id`] share this one rule, so a runner can never keep a value the adapter then refuses (that would leave
/// the game offline for good).
pub const DEVICE_SECRET_LEN: usize = 32;

/// Domain tag of the derivation. Changing it, or anything else that feeds the hash, gives every player a new account (see the
/// golden vector in the tests).
const DEVICE_ID_CONTEXT: &str = "ironhold device id v1";

/// True for exactly the values [`device_secret_from_bytes`] makes: 32 lowercase hex characters.
pub fn is_valid_device_secret(secret: &str) -> bool {
    secret.len() == DEVICE_SECRET_LEN && secret.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A new device secret from 16 random bytes the runner took from its OS or browser random source.
pub fn device_secret_from_bytes(bytes: &[u8; 16]) -> String {
    hex(bytes)
}

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

    /// `{game_id}:` plus 32 hex characters of a one-way hash of the game and the stored secret. The server only ever sees this
    /// id, never the secret, so an id captured from one game opens nothing in another. The readable prefix stays because the
    /// server picks the game from it. All inputs are known to the client: this does not stop a script that can read the secret.
    pub fn device_id(&self, secret: &str) -> Result<String, String> {
        if !is_valid_device_secret(secret) {
            return Err("the device secret must be 32 lowercase hex characters".to_string());
        }
        let mut material = Vec::with_capacity(self.0.len() + 1 + secret.len());
        material.extend_from_slice(self.0.as_bytes());
        material.push(0); // game ids are [a-z0-9_] and the secret is hex, so neither can contain it: no two inputs collide
        material.extend_from_slice(secret.as_bytes());
        let hash = blake3::derive_key(DEVICE_ID_CONTEXT, &material);
        Ok(format!("{}:{}", self.0, hex(&hash[..16])))
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
    }

    const SECRET: &str = "00112233445566778899aabbccddeeff";

    #[test]
    fn the_device_id_is_a_per_game_hash_of_the_secret() {
        let a = GameNs::new("my_game").unwrap().device_id(SECRET).unwrap();
        let b = GameNs::new("other_game").unwrap().device_id(SECRET).unwrap();
        let c = GameNs::new("my_game").unwrap().device_id("ffeeddccbbaa99887766554433221100").unwrap();
        assert_eq!(a, GameNs::new("my_game").unwrap().device_id(SECRET).unwrap(), "stable for the same inputs");
        assert_ne!(a.split(':').nth(1), b.split(':').nth(1), "another game gives another hash, not just another prefix");
        assert_ne!(a, c, "another secret gives another id");
        let (prefix, hash) = a.split_once(':').unwrap();
        assert_eq!(prefix, "my_game");
        assert_eq!(hash.len(), 32);
        assert!(hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
        assert!(!a.contains(SECRET), "the secret itself is never in the id");
    }

    /// The compatibility contract (the value was cross-checked with the independent Python `blake3` package): changing the derivation gives every player a new account. If this fails on purpose, that is
    /// a migration (see planning/features/device_id_binding.md), not a test to update.
    #[test]
    fn golden_vector() {
        assert_eq!(GameNs::new("my_game").unwrap().device_id(SECRET).unwrap(), "my_game:7f4e484e070f15ac68effb5c23d7fb8f");
    }

    #[test]
    fn only_a_well_formed_secret_is_accepted() {
        let ns = GameNs::new("my_game").unwrap();
        for bad in ["", "x", "0123-abcd-4567", "6f1c2b9e-0000-4a11-9a00-ironholdv2a", "00112233445566778899AABBCCDDEEFF", "00112233445566778899aabbccddeef", "00112233445566778899aabbccddeeff0"] {
            assert!(ns.device_id(bad).is_err(), "{bad:?}");
            assert!(!is_valid_device_secret(bad), "{bad:?}");
        }
        assert!(is_valid_device_secret(SECRET));
        assert!(is_valid_device_secret(&device_secret_from_bytes(&[0xab; 16])));
    }
}
