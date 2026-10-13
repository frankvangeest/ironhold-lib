---
name: stakeholder-wishlist-tracking
description: My top-5 architect items in planning/stakeholder_priority_list.md — 1st (db1ede0), 2nd (ce77bdd, 2026-10-01) and 3rd-edition (929b7d1, 2026-10-13) lists and their statuses
metadata:
  type: project
---

`planning/stakeholder_priority_list.md` holds a 5-stakeholder snapshot, re-run on request. My section is "System-Architect — stability, maintainability, future-proofing".

**1st edition (db1ede0, 2026-09-03):** float divergence (Changed), Action deny_unknown_fields (Shipped 33842ff), camera/input on PrefabDef (open, Icebox), test trust (Changed), spawn_scene_v2 16 params (open).

**2nd edition (ce77bdd) → status at 3rd edition (929b7d1, 2026-10-13):** all five STILL OPEN.
1. Determinism harness before ocean — open; but ocean batch also not started (0 ocean code in src), and D1 (9e167ca) + D2 (7637605) shipped as groundwork; D5 two-App smoke test queued as cheap first slice.
2. Schedule-ordering contract — Changed: became D3, plan Ready 2026-10-02, still unbuilt; zero SystemSets in src.
3. UiPointerCaptured — open (moved Bugs→Queued; no code).
4. Loader failure visibility — open in-engine; play.html now reports pre-first-frame failures (b5c1800) but a parse failure after frame 1 still hangs; check_project_loaded has no Failed arm for the ProjectConfig handle.
5. scene_manager monolith — open; spawn_scene_v2 still 16 params, scene_loader.rs 3570→3686 lines, folder 9287.

**3rd-edition top-5:** (1) build D3 now; (2) "invalid data fails loudly and safely" — loader Failed arms + runtime-total validated fields (see [[runtime-validate-is-advisory]]); (3) input ownership incl. touch for mobile_ui_demo; (4) cheapest determinism measurement (D5) + make is_presentation_only a real contract; (5) scene_manager monolith.

Observed 2026-10-13: only 26 of 204 non-merge commits since ce77bdd touched crates/ironhold_core/src — the period went mostly to OpenCode tooling, docs/CLAUDE.md split, planning. Backlog `## Active` is empty (drift from 2nd edition fixed).

**Why:** Frank periodically asks each persona to grade progress against its own wishlist.

**How to apply:** Re-verify against `git log <snapshot>..HEAD` and current backlog before quoting. See [[determinism-networking]], [[schedule-ordering-mechanism]], [[panel-input-blocking]], [[schema-tightening-blast-radius]], [[fragile-modules]].
