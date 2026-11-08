---
name: device-id-binding-facts
description: Plan-review facts for device_id_binding.md (rounds 1+2, 2026-11-07) - web ?backend= fake-only, legacy uuids in DB, blake3 in tree, homelab deploy probe uses a 20-hex id, ironhold_web cannot host native tests, secret-rule must be shared runner/ns
metadata:
  type: project
---

Plan reviews of `planning/features/device_id_binding.md`: round 1 at `fd6ce9b` (Needs more design work), round 2 at `e37463e`
(2026-11-07, migration A + no endpoint in hash; verdict: task-list additions only, no further design round).

- **The "redirected endpoint" hole is already closed on web**: `ironhold_web/src/lib.rs` drops any `?backend=` that is not `fake://`.
  Only other writer of `AppOptions::backend_endpoint` is native `--backend` (main.rs). `?project=` only reaches same-origin files.
- **Who a client-derived hash protects against:** holders of ONE game's wire id without the stored secret. NOT a whole-DB/console/
  Cloudflare leak, NOT another page on the same web origin (github.io shared origin).
- **Legacy `{game}:{uuid}` rows stay in Nakama's device table** -> rotate the stored value + purge. Homelab Postgres backups keep them
  until retention expires (a "dump contains no raw uuid" criterion only holds for the live DB).
- **Hash in tree:** `blake3 1.8.3` via bevy_asset/bevy_animation in both trees; `derive_key(&str, &[u8]) -> [u8;32]`. `sha2` NOT in lock.
  `ns.rs` is not behind the `client` feature, so the CLI (`default-features = false`) links whatever ns.rs uses.
- **Homelab `infrastructure/proxmox/deploy-nakama-modules.sh` `probe()`** signs in with `{game}:0123456789abcdef0123` (20 hex,
  create=false) and rolls the deploy back unless the first hosted game is NOT 403. A strict 32-hex server check makes every deploy
  roll back until the probe uses 32 hex (the copy on pmve is `/root/deploy-nakama-modules.sh`, hand-copied). Unknown-game probe stays
  valid only if the Lua checks the game before the format.
- **`ironhold_web` cannot run native `cargo test`**: web-sys is a wasm32-only dep and lib.rs uses it unconditionally. A "pin the
  filter" test needs the predicate moved to a natively testable crate (core/ironhold_nakama) or a wasm test target.
- Runner "replace if unusable" rule (16..=100 of [A-Za-z0-9-]) and ns.rs's rule must be ONE shared predicate, or a stored value the
  runner keeps but ns rejects makes `NakamaBackend::new` fail forever (permanent offline).
- Test fixtures depending on the secret rule: adapter_flow.rs `"0123-abcd-4567"` (14 chars, 6 sites), contract.rs `unique("contract-a")`
  via NakamaBackend, live_lan.rs fixed `"...-ironholdv2a"` (non-hex). raw_session sites 127/158/192/193/206 need 32-hex ids.
- Refresh path: SessionRefresh bypasses `before_authenticate_device`, so legacy sessions live until their accounts are purged;
  re-auth reuses the constructed `self.device_id`. A re-auth refusal surfaces as the waiting call's failure (e.g. submit_failed),
  not `backend.sign_in_failed`.
- Nakama JWT carries uid/usn/vars, not the device id; leaderboard rows carry owner_id/username only.


Code review of `c445b6e` (2026-11-08, feature/device_id_binding):
- Golden vector `my_game` + `00112233445566778899aabbccddeeff` -> `my_game:7f4e484e070f15ac68effb5c23d7fb8f` independently
  re-derived with a standalone blake3 =1.8.3 crate (no Python blake3 on this machine). blake3 as a direct dep is a zero
  feature delta: bevy_asset uses default-features=false but bevy_animation already turns on `default`/`std`.
- `is_fake_endpoint` is ALSO what `choose_backend` (runtime/backend.rs) uses to pick the fake backend, so the web `?backend=`
  filter is correct by construction (a page can only pick endpoints that never touch a network) - reusing it beat a new fn.
- adapter_flow's mock `serve()` handler receives the request body, so "the wire carries the hash, not the secret" is cheaply
  assertable at adapter level; the shipped diff only unit-tests it in ns.rs.
- Homelab probe still sent the 20-hex id on 2026-11-08 (not yet changed).

**Why:** these decide whether the binding is honest about what it closes and whether release day works.
**How to apply:** any review of device-id, account-migration, multi-tenant hosting, cloud-save identity, or Nakama module-deploy plans.
See [[backend-abstraction-review-facts]].
