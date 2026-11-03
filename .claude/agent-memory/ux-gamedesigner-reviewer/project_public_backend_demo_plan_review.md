---
name: public-backend-demo-plan-review
description: public_backend_demo plan (2026-10-30) + P3 impl review (2026-11-03): friendly-down recipe, sign_in_failed overloaded, gateway-error doc contradiction, snippet drift, visitor privacy copy
metadata:
  type: project
---

Plan `planning/features/public_backend_demo.md` reviewed 2026-10-30 (P2 ready, P3 not). P3 impl reviewed 2026-11-03: nothing blocking.

Open after P3 impl review (re-check if touched again):
- `backend.sign_in_failed` means BOTH "server rejected" (wrong key / game_id not in ironhold_games.lua) and a gateway 5xx with CORS
  (native, last_error `server answered HTTP 5xx`, no row in docs/20 last_error table). The demo + docs/30 recipe map it to
  "Server unavailable. Press Sign in to retry." -> the copy trap (renamed game_id) shows "unavailable" forever.
- docs/20 ~5111 and docs/30 ~460 say a gateway error page -> sign_in_failed; docs/20 ~5150/5157 say browser gets `server unreachable`
  (no CORS). Needs "web: unreachable; native: sign_in_failed + server answered HTTP n".
- "validate rejects writing backend.*" (docs/30 ~459, state_machine.ron comment) is wrong: it is a `--strict` warning `reserved_variable_write`.
- backend_demo_online main.scene.ron says account block = ui/signin.ui.ron, but snippet lacks demo_msg_label (drift).
- Visitor copy: on-screen "see the card" dead-ends for deep links; card says "offline", screen says "unavailable" + raw `signed_out`;
  nothing says WHAT is stored (random name, scores, device id); "note your name before clearing site data" missing.

Earlier plan items now resolved: offline vs down distinguished in docs; publishing path section exists in docs/20; console lines explained.

**Why:** first public deployment; visitors (not designers) see last_error text directly.
**How to apply:** when backend error mapping or the demo changes, check the 4 surfaces: docs/20 last_error table + console notes,
docs/30 Online events recipe, backend_demo_online RON comments, index.html card. Related:
[[backend-abstraction-plan-review]], [[project_ui_label_button_font_and_clip]], [[project_em_dash_font_glyph_gap]].
