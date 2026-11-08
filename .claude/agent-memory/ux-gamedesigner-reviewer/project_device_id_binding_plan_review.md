---
name: device-id-binding-plan-review
description: device_id_binding plan r1+r2 + impl review (2026-11-08, f094ff2..c445b6e): secret is per browser ORIGIN so moving host address = new accounts (undocumented); stale-pkg cause missing from out-of-date row
metadata:
  type: project
---

Plan `planning/features/device_id_binding.md`. r2 **Ready**. Frank decided: migration A (clean break, storage key
`ironhold.device_secret`, purge legacy accounts, wipe `best`), endpoint NOT hashed. Wire id = `{game_id}:` + 32 hex
(blake3 of game_id + secret); server lua rejects other suffixes with "out of date: reload the page or update the game" (403 -> text passes through to backend.last_error).

Impl review (2026-11-08) found, docs/20 ~5115-5166:
- Secret lives in localStorage = per browser profile AND per web address (origin). Docs say "per browser" only; the
  recommended fix "give each game its own address" (custom domain) silently gives every existing player a new account.
- game_id feeds the account code; game_id defaults to project_id, so renaming either = all players start over. Not in field row.
- out-of-date row omits the designer-side cause: hosted pkg/ is still the old engine build (reload won't help).
- Upgrading para: old scores stay on boards as ghost entries unless admin purges; server module + build upgrade order unstated.
- Runtime storage-failure texts still say "device id" while docs say "device secret" (left verbatim on purpose by plan).
- index.html card calls the secret a "random key" (clashes with server key). Secret IS written at page load (factory runs
  after backend.ron loads), so card's "when the page loads" is accurate; docs/30 ~499 "never creates an account" still true.
- Dated reset notice on card + homelab steps deliberately deferred to release day.

**How to apply:** on later backend/hosting doc changes, check the per-origin consequence and the game_id-rename consequence are stated.
Related: [[public-backend-demo-plan-review]], [[backend-v3-lobby-plan-review]].
