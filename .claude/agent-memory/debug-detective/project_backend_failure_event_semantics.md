---
name: backend-failure-event-semantics
description: Nakama adapter failure-event facts - SignOut mid-sign-in emits sign_in_failed("signed out"); failed renewal never says "session expired"; down-while-online emits no unreachable
metadata:
  type: project
---

Facts about which events and `backend.last_error` text each failure path really produces (verified by reading `ironhold_nakama/src/client.rs` + `runtime/backend.rs` at feature/public_backend_demo P3, 2026-11-03):

- **SignOut while a SignIn is in flight**: `cancel_all("signed out")` answers the SignIn as `SignInFailed`, so the rules see `backend.sign_in_failed` (lower request id, sorted first) and then `backend.signed_out` in the same frame. A "server down" message bound to `sign_in_failed` lies unless `backend.signed_out` clears it afterwards.
- **`the session expired, sign in again`** shows only when the renewal *succeeded* and the retried call got a 401 again. A *failed* renewal gives `server unreachable` (transport), `the server rejected the sign-in` (parse_session maps 401), or `server answered HTTP N`. Engine status stays `online` in all of these.
- **Server down while signed in**: Submit/Fetch fail as `score.submit_failed:` / `leaderboard.list_failed:`, never `backend.unreachable` (that is only SignIn/SignOut). Status still reads `online`.
- **Native vs web**: a 502/530/429 gateway page is a transport error only in the browser (no CORS headers). Native gets a real response: `server answered HTTP 530`, and on SignIn that is `sign_in_failed`, not `unreachable`.

**Why:** in a doc or demo state machine, a "friendly message" recipe written from the event names alone gets these wrong.
**How to apply:** when you review backend RON or docs, trace each claim through `client.rs` `handle`/`handle_renewal`/`cancel_all`. See [[schema-validate-errors-are-nonfatal]] for the general rule that silent paths are common.
