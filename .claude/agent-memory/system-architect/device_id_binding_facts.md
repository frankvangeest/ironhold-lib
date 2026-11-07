---
name: device-id-binding-facts
description: Plan-review facts for device_id_binding.md (fd6ce9b, 2026-11-07) - web ?backend= is already fake-only, legacy raw uuids stay in Nakama DB, blake3 already in both trees, contract tests build non-hex ids
metadata:
  type: project
---

Plan review of `planning/features/device_id_binding.md` at `fd6ce9b` (2026-11-07), verdict: Needs more design work (one short round).

- **The "redirected endpoint" hole is already closed on web**: `ironhold_web/src/lib.rs` drops any `?backend=` that is not `fake://`
  (decision in backend_abstraction.md v2). Native `--backend` needs local exec (already owns the uuid file). `?project=` only reaches
  same-origin files. So endpoint-in-hash buys ~nothing today and orphans every account on a hostname move / LAN->tunnel.
- **Who a client-derived hash protects against:** holders of ONE game's wire id without the uuid (a support HAR/devtools capture, an http
  LAN sniffer, a future per-tenant admin export). NOT a whole-DB/console/Cloudflare leak of a shared Nakama: that holds every game's id.
- **Legacy `{game}:{uuid}` rows stay in Nakama's device table** after a clean break -> raw uuids still derivable from a DB dump. Fix:
  rotate the stored uuid (free when accounts are orphaned anyway) and/or purge legacy accounts. Alternative migration: admin one-shot
  server-side rewrite (server knows the uuid from the legacy id; needs a hash the server can compute: sha256 via `nk.sha256_hash`/SQL).
- **Hash in tree:** `blake3 1.8.3` is unconditional in both wasm32 and native trees (bevy_asset/bevy_animation): zero size cost,
  `derive_key` gives domain separation. `sha2` is NOT in the lock. `ring` is native-only (rustls).
- `NakamaBackend::new` uses `base = endpoint.trim_end_matches('/')` (no lowercasing, path kept); schema only checks the http(s) prefix.
- Contract tests sign in raw with `alpha:{unique(..)}` (`adv-<nanos>`) at 5 sites: a server 32-hex check breaks them; adapter_flow's
  mock ignores the body; ns.rs unit test asserts the exact legacy string. No `ironhold_nakama` CLAUDE.md exists.
- Nakama JWT carries uid/usn/vars, not the device id; leaderboard rows carry owner_id/username only. Wire id is visible to: DB/console,
  Cloudflare TLS terminator, the player's own devtools.

**Why:** these decide whether the binding is honest about what it closes and what migration costs.
**How to apply:** any review of device-id, account-migration, multi-tenant hosting or cloud-save identity plans. See [[backend-abstraction-review-facts]].
