---
name: device-id-binding-plan-review
description: device_id_binding plan-review 2026-11-07: endpoint-in-hash makes a server move wipe accounts; docs/20 ~5125 `{game_id}:{device id}` becomes false; legacy refusal must not reuse "unknown game"
metadata:
  type: project
---

Plan `planning/features/device_id_binding.md` (Draft, fd6ce9b) reviewed 2026-11-07: Needs more design work (small).

Open items to re-check on the next pass / implementation review:
- Endpoint is a hash input: moving a game to another hostname (LAN -> tunnel, staging -> public, http -> https, new domain) silently
  gives every player a new empty account. Web endpoint already only comes from backend.ron (docs/20 ~5114), so the redirect threat
  is mostly native `--backend`. Recommended: decide now (id format is versioned v1): drop endpoint, or an optional `account_scope`.
  Either way a "Moving to another server" paragraph in docs/20 next to "Rotating the key".
- docs/20 ~5125 "`{game_id}:{device id}`" becomes false; ~5115 Device id row should split stored vs sent.
- Legacy-id refusal must not reuse `unknown game` (its docs/20 fix row says re-run `ironhold backend config` - wrong advice).
- Plan contradicts itself on server-check timing (Option A "same release" vs Server section/Tasks "after migration window").
- Clean break: card's "note your name first" does not help; orphaned legacy rows keep the top-5; legacy accounts (raw uuid on server)
  are pure retention -> recommend wiping board + deleting legacy accounts at cutover, one dated line on the card.
- Same-origin limit sentence is programmer-speak; designer needs "all games under yourname.github.io share one address".
- Private-window / blocked-storage path: unaffected (fails before derivation).

**Why:** last chance to change account identity cheaply (before lobbies on public + cloud save).
**How to apply:** check docs/20 backend section, index.html backend_demo_online card, backend_demo_online note labels together.
Related: [[public-backend-demo-plan-review]], [[backend-v3-lobby-plan-review]].
