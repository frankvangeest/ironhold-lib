---
name: device-id-binding-plan-review
description: device_id_binding plan-review r1+r2 (2026-11-07): r2 Ready; residual = docs/30 ~477 + demo comment cause lists, native-neutral reject text, card "device id saved in browser" goes false, notice removal date
metadata:
  type: project
---

Plan `planning/features/device_id_binding.md`. r1 (fd6ce9b): Needs more design work. r2 (e37463e): **Ready**, residual nits only.
Frank decided: migration A (clean break + rotate storage key to `device_secret` + purge legacy accounts + wipe `best`), endpoint NOT hashed.

Residual items to check at implementation review:
- Reject text must be native-neutral and fit the 640px error_label (~58 chars at ~11px/char): plan's web-only
  "this game version is out of date, reload the page" (49) fits; recommended "out of date: reload the page or update the game" (47).
- docs/30 ~477-478 and backend_demo_online/logic/state_machine.ron ~23 list refused-sign-in causes (wrong key, game not hosted): add out-of-date build. Not in plan Tasks.
- index.html card ~930-932 "the device id is also saved in this browser" becomes false (browser keeps a secret, server gets a derived code): make the wording task mandatory, not optional. Keep "note your name first" (still serves deletion requests).
- Dated reset line needs a removal task/date, and can state the privacy plus (old ids deleted) and that the random name changes.
- Board wipe after the ~1-day lag also erases scores of new-client accounts made in the lag; check whether Nakama account deletion already removes those users' records (then the wipe is unneeded).
- Runner storage-failure messages are quoted by prefix in docs/20 ~5159 ("would not store a device id"): keep text verbatim or update the row.
- Pending deletion-request GitHub issues: answer/close them after the purge.

**Why:** last cheap moment to change account identity (before lobbies on public + cloud save).
**How to apply:** at implementation review grep docs/20 ~5115-5161, docs/30 ~477, index.html card, demo state_machine.ron for "device id"/cause lists.
Related: [[public-backend-demo-plan-review]], [[backend-v3-lobby-plan-review]].
