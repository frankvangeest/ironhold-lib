---
name: public-backend-demo-plan-review
description: public_backend_demo plan review 2026-10-30 (server_key in backend.ron, backend_demo_online on Pages vs homelab Nakama); open designer/visitor gaps to re-check at impl review
metadata:
  type: project
---

Plan `planning/features/public_backend_demo.md` reviewed 2026-10-30 -> verdict: P2 ready with doc additions, P3 not ready.

Re-check at implementation review:
- Plan conflates `backend.offline` (no usable backend config) with a down server (`backend.unreachable` / `backend.sign_in_failed`).
  Demo has no auto SignIn (backend.ready line commented out), so "offline message on page load" acceptance can't happen as written.
- Behind Cloudflare Tunnel a down homelab answers HTTP 502/530 (or WAF 429), not a transport error. Adapter maps non-2xx to
  "server answered HTTP 5xx" via SignInFailed (protocol.rs ok_json); in browsers missing CORS on CF error pages may turn it into
  "server unreachable". Rate-limited visitor would read "server unreachable". Needs a live check + troubleshooting rows.
- Visitor-facing text today = raw `backend.status` tokens ("signed_out") and raw last_error; friendly copy needs a
  SetVariable bridge on backend.unreachable / sign_in_failed (no recipe in docs/30 yet).
- Copy trap: public server only accepts game_ids in deployed ironhold_games.lua; a designer copying backend_demo_online and
  renaming game_id (as duplicate_game_id pushes) gets "the server rejected the sign-in". No designer publishing path exists in
  any doc (README only links Pages).
- Stale after P2: backend_demo/backend.ron comment "(the page address picks the key)"; docs/20 play.html?nakama_key example and
  the "no Nakama server key" fix row; a stale ?nakama_key= in a shared link overrides a rotated in-file key.
- "Rotating means rebuilding clients": for web, RON is served next to the build, so it is "republish + cache", verify wording.
- Deletion-on-request needs an identifier the visitor can see: the random `backend.display_name` (unique Nakama username).

**Why:** first public deployment; visitors (not designers) see last_error text directly.
**How to apply:** at impl review, read backend_demo_online scene/logic + docs/20 Nakama section against these. Related:
[[backend-abstraction-plan-review]], [[project_ui_label_button_font_and_clip]], [[project_em_dash_font_glyph_gap]].
